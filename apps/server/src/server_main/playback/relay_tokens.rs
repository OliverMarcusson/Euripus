use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub(in crate::server_main) struct RelayClaims {
    pub(in crate::server_main) sub: String,
    pub(in crate::server_main) pid: String,
    pub(in crate::server_main) url: String,
    pub(in crate::server_main) kind: RelayAssetKind,
    pub(in crate::server_main) exp: usize,
}

#[derive(Debug)]
pub(in crate::server_main) struct RelayToken {
    pub(in crate::server_main) token: String,
    pub(in crate::server_main) expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub(in crate::server_main) struct RelayTokenQuery {
    pub(in crate::server_main) token: String,
}

pub(in crate::server_main) struct ValidatedRelayToken {
    pub(in crate::server_main) user_id: Uuid,
    pub(in crate::server_main) profile_id: Uuid,
    pub(in crate::server_main) upstream_url: Url,
    pub(in crate::server_main) expires_at: DateTime<Utc>,
}

pub(in crate::server_main) async fn validate_relay_token(
    state: &AppState,
    token: &str,
    expected_kind: RelayAssetKind,
) -> Result<ValidatedRelayToken, AppError> {
    let relay = decode_relay_token(&state.config, token, expected_kind)?;
    let cache_key = (relay.profile_id, relay.user_id);
    let now = Instant::now();
    let cached_expiry = state
        .relay_profile_cache
        .get(&cache_key)
        .map(|expiry| *expiry);
    if let Some(expiry) = cached_expiry {
        if expiry > now {
            return Ok(relay);
        }
        state.relay_profile_cache.remove(&cache_key);
    }

    let valid_profile = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
          SELECT 1
          FROM provider_profiles
          WHERE id = $1 AND user_id = $2
        )
        "#,
    )
    .bind(relay.profile_id)
    .bind(relay.user_id)
    .fetch_one(&state.pool)
    .await?;
    if !valid_profile {
        return Err(AppError::Unauthorized);
    }

    let cache_ttl = relay
        .expires_at
        .signed_duration_since(Utc::now())
        .to_std()
        .map(|duration| duration.min(RELAY_PROFILE_CACHE_TTL))
        .unwrap_or(RELAY_PROFILE_CACHE_TTL);
    state.relay_profile_cache.insert(cache_key, now + cache_ttl);

    Ok(relay)
}

pub(in crate::server_main) fn issue_relay_token(
    state: &AppState,
    user_id: Uuid,
    profile_id: Uuid,
    upstream_url: &str,
    kind: RelayAssetKind,
    expires_at: Option<DateTime<Utc>>,
) -> Result<RelayToken, AppError> {
    let expires_at = expires_at
        .unwrap_or_else(|| Utc::now() + ChronoDuration::minutes(state.config.relay_token_minutes));
    let claims = RelayClaims {
        sub: user_id.to_string(),
        pid: profile_id.to_string(),
        url: upstream_url.to_string(),
        kind,
        exp: expires_at.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.relay_signing_secret.as_bytes()),
    )
    .map_err(|error| AppError::Internal(anyhow!(error)))?;

    Ok(RelayToken { token, expires_at })
}

pub(in crate::server_main) fn decode_relay_token(
    config: &Config,
    token: &str,
    expected_kind: RelayAssetKind,
) -> Result<ValidatedRelayToken, AppError> {
    let claims = decode::<RelayClaims>(
        token,
        &DecodingKey::from_secret(config.relay_signing_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;

    if claims.kind != expected_kind {
        return Err(AppError::Unauthorized);
    }

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
    let profile_id = Uuid::parse_str(&claims.pid).map_err(|_| AppError::Unauthorized)?;
    let upstream_url = Url::parse(&claims.url).map_err(|_| AppError::Unauthorized)?;
    if !matches!(upstream_url.scheme(), "http" | "https") {
        return Err(AppError::Unauthorized);
    }

    let expires_at =
        DateTime::<Utc>::from_timestamp(claims.exp as i64, 0).ok_or(AppError::Unauthorized)?;

    Ok(ValidatedRelayToken {
        user_id,
        profile_id,
        upstream_url,
        expires_at,
    })
}

pub(in crate::server_main) fn relay_asset_kind_for_url(url: &Url) -> RelayAssetKind {
    if url
        .path_segments()
        .and_then(|segments| segments.last())
        .is_some_and(|segment| segment.ends_with(".m3u8"))
    {
        RelayAssetKind::Hls
    } else {
        RelayAssetKind::Raw
    }
}

pub(in crate::server_main) fn relay_url_for_token(
    base_url: &Url,
    kind: RelayAssetKind,
    token: &str,
) -> Result<String, AppError> {
    let mut url = base_url
        .join(match kind {
            RelayAssetKind::Hls => "/api/relay/hls",
            RelayAssetKind::Raw => "/api/relay/raw",
            RelayAssetKind::Asset => "/api/relay/asset",
        })
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    url.query_pairs_mut().append_pair("token", token);
    Ok(url.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_app_state() -> AppState {
        AppState {
            pool: PgPoolOptions::new()
                .connect_lazy("postgres://euripus:euripus@localhost/euripus")
                .expect("lazy pool"),
            config: Arc::new(Config {
                bind_address: "127.0.0.1:4000".parse().expect("bind address"),
                database_url: "postgres://euripus:euripus@localhost/euripus".to_string(),
                jwt_secret: "test-jwt-secret".to_string(),
                relay_signing_secret: "test-relay-secret".to_string(),
                encryption_key: *b"0123456789abcdef0123456789abcdef",
                access_token_minutes: 15,
                refresh_token_days: 7,
                relay_token_minutes: 30,
                daily_sync_hour_local: 6,
                public_origin: Some(Url::parse("https://app.example.com").expect("public origin")),
                allowed_origins: vec!["https://app.example.com".to_string()],
                browser_cookie_secure: true,
                vpn_enabled: false,
                vpn_provider_name: None,
                meilisearch_url: None,
                meilisearch_api_key: None,
                admin_password: None,
            }),
            provider_http_client: reqwest::Client::new(),
            relay_http_client: reqwest::Client::new(),
            meili: None,
            meili_readiness: Arc::new(RwLock::new(MeiliReadiness::Disabled)),
            meili_schema_ready: Arc::new(RwLock::new(true)),
            meili_bootstrapping_users: Arc::new(DashSet::new()),
            search_lexicons: Arc::new(DashMap::new()),
            session_cache: Arc::new(DashMap::new()),
            relay_profile_cache: Arc::new(DashMap::new()),
            channel_visibility_cache: Arc::new(DashMap::new()),
            receiver_channels: Arc::new(DashMap::new()),
        }
    }

    #[tokio::test]
    async fn decode_relay_token_rejects_tampered_tokens() {
        let state = sample_app_state();
        let issued = issue_relay_token(
            &state,
            Uuid::from_u128(5),
            Uuid::from_u128(6),
            "https://provider.example.com/live/42.m3u8",
            RelayAssetKind::Hls,
            Some(Utc::now() + ChronoDuration::minutes(10)),
        )
        .expect("issue relay token");
        let tampered = format!("{}x", issued.token);

        let result = decode_relay_token(&state.config, &tampered, RelayAssetKind::Hls);

        assert!(matches!(result, Err(AppError::Unauthorized)));
    }

    #[tokio::test]
    async fn decode_relay_token_rejects_wrong_asset_kind() {
        let state = sample_app_state();
        let issued = issue_relay_token(
            &state,
            Uuid::from_u128(5),
            Uuid::from_u128(6),
            "https://provider.example.com/live/42.m3u8",
            RelayAssetKind::Hls,
            Some(Utc::now() + ChronoDuration::minutes(10)),
        )
        .expect("issue relay token");

        let result = decode_relay_token(&state.config, &issued.token, RelayAssetKind::Raw);

        assert!(matches!(result, Err(AppError::Unauthorized)));
    }
}
