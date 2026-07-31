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
#[path = "relay_tokens_tests.rs"]
mod tests;
