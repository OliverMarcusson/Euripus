use self::resolve::{
    PlaybackSourceResponse, PlaybackStreamFormat, PlaybackTarget, ProgramPlaybackBehavior,
    ProgramPlaybackRow, determine_program_playback_behavior, output_format_as_str,
    playback_source_for_mode, resolve_effective_playback_format,
    resolve_effective_playback_format_for_target, unsupported_playback,
};
use super::*;

pub(super) mod relay_tokens;
pub(super) mod resolve;

const BROWSER_HLS_UNSUPPORTED_REASON: &str =
    "This provider stream could not be verified for browser HLS playback. Try a receiver/native target instead.";

#[derive(Debug, FromRow)]
struct ChannelPlaybackRecord {
    id: Uuid,
    profile_id: Uuid,
    name: String,
    remote_stream_id: i32,
    stream_extension: Option<String>,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
    playback_mode: String,
}

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/playback/channel/{id}", post(play_channel))
        .route("/playback/program/{id}", post(play_program))
}

async fn play_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    Ok(Json(
        resolve_channel_playback_source(&state, &headers, auth.user_id, id).await?,
    ))
}

async fn play_program(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    Ok(Json(
        resolve_program_playback_source(&state, &headers, auth.user_id, id).await?,
    ))
}

async fn resolve_channel_playback_source(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_channel_playback_source_for_target(state, headers, user_id, id, PlaybackTarget::Browser)
        .await
}

pub(in crate::server_main) async fn resolve_channel_playback_source_for_receiver(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    receiver_app_kind: &str,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_channel_playback_source_for_target(
        state,
        headers,
        user_id,
        id,
        playback_target_for_receiver_app(receiver_app_kind),
    )
    .await
}

async fn resolve_channel_playback_source_for_target(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    target: PlaybackTarget,
) -> Result<PlaybackSourceResponse, AppError> {
    let record = sqlx::query_as::<_, ChannelPlaybackRecord>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.remote_stream_id,
          c.stream_extension,
          p.base_url,
          p.username AS provider_username,
          p.password_encrypted,
          p.output_format,
          p.playback_mode
        FROM channels c
        JOIN provider_profiles p ON p.id = c.profile_id
        WHERE c.user_id = $1 AND c.id = $2
        "#,
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Channel not found".to_string()))?;

    let credentials = playback_credentials(state, &record)?;
    let browser_hls_preflight_required = target_requires_browser_hls_preflight(
        target,
        &record.output_format,
        record.stream_extension.as_deref(),
    );
    let format = resolve_effective_playback_format_for_target(
        target,
        &record.output_format,
        record.stream_extension.as_deref(),
    )?;
    let url = xtreme::build_live_stream_url(
        &credentials,
        record.remote_stream_id,
        Some(output_format_as_str(format)),
    )?;
    touch_recent(&state.pool, user_id, record.id).await?;

    finalize_playback_source(
        state,
        headers,
        user_id,
        record.profile_id,
        target,
        &record.playback_mode,
        &record.name,
        url,
        true,
        false,
        format,
        None,
        browser_hls_preflight_required,
    )
    .await
}

async fn resolve_program_playback_source(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_program_playback_source_for_target(state, headers, user_id, id, PlaybackTarget::Browser)
        .await
}

pub(in crate::server_main) async fn resolve_program_playback_source_for_receiver(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    receiver_app_kind: &str,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_program_playback_source_for_target(
        state,
        headers,
        user_id,
        id,
        playback_target_for_receiver_app(receiver_app_kind),
    )
    .await
}

async fn resolve_program_playback_source_for_target(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    target: PlaybackTarget,
) -> Result<PlaybackSourceResponse, AppError> {
    let row = sqlx::query_as::<_, ProgramPlaybackRow>(
        r#"
        SELECT
          p.title,
          p.start_at,
          p.end_at,
          p.can_catchup,
          p.profile_id,
          c.id AS channel_id,
          c.remote_stream_id,
          c.stream_extension,
          c.name AS channel_name,
          c.has_catchup,
          pr.base_url,
          pr.username AS provider_username,
          pr.password_encrypted,
          pr.output_format,
          pr.playback_mode
        FROM programs p
        LEFT JOIN channels c ON c.id = p.channel_id
        LEFT JOIN provider_profiles pr ON pr.id = p.profile_id
        WHERE p.user_id = $1 AND p.id = $2
        "#,
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Program not found".to_string()))?;

    let behavior = determine_program_playback_behavior(&row, Utc::now());

    let Some(channel_id) = row.channel_id else {
        return Ok(unsupported_playback(
            &row.title,
            "This program is not mapped to a playable channel.",
        ));
    };
    touch_recent(&state.pool, user_id, channel_id).await?;

    match behavior {
        ProgramPlaybackBehavior::Live => {
            let credentials = XtreamCredentials {
                base_url: row.base_url,
                username: row.provider_username,
                password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
                output_format: row.output_format,
            };
            let browser_hls_preflight_required = target_requires_browser_hls_preflight(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            );
            let format = resolve_effective_playback_format_for_target(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            )?;
            let url = xtreme::build_live_stream_url(
                &credentials,
                row.remote_stream_id,
                Some(output_format_as_str(format)),
            )?;

            finalize_playback_source(
                state,
                headers,
                user_id,
                row.profile_id,
                target,
                &row.playback_mode,
                &row.channel_name,
                url,
                true,
                false,
                format,
                None,
                browser_hls_preflight_required,
            )
            .await
        }
        ProgramPlaybackBehavior::Catchup => {
            let credentials = XtreamCredentials {
                base_url: row.base_url,
                username: row.provider_username,
                password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
                output_format: row.output_format,
            };
            let browser_hls_preflight_required = target_requires_browser_hls_preflight(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            );
            let format = resolve_effective_playback_format_for_target(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            )?;
            let url = xtreme::build_catchup_url(
                &credentials,
                row.remote_stream_id,
                Some(output_format_as_str(format)),
                row.start_at,
                row.end_at,
            )?;

            finalize_playback_source(
                state,
                headers,
                user_id,
                row.profile_id,
                target,
                &row.playback_mode,
                &row.title,
                url,
                false,
                true,
                format,
                None,
                browser_hls_preflight_required,
            )
            .await
        }
        ProgramPlaybackBehavior::Unsupported(reason) => {
            Ok(unsupported_playback(&row.title, reason))
        }
    }
}

fn playback_credentials(
    state: &AppState,
    record: &ChannelPlaybackRecord,
) -> Result<XtreamCredentials> {
    Ok(XtreamCredentials {
        base_url: record.base_url.clone(),
        username: record.provider_username.clone(),
        password: decrypt_secret(&state.config.encryption_key, &record.password_encrypted)?,
        output_format: record.output_format.clone(),
    })
}

fn playback_target_for_receiver_app(app_kind: &str) -> PlaybackTarget {
    if app_kind == "receiver-android-tv" {
        PlaybackTarget::ReceiverAndroidTv
    } else {
        PlaybackTarget::ReceiverWeb
    }
}

fn target_requires_browser_hls_preflight(
    target: PlaybackTarget,
    output_format: &str,
    legacy_stream_extension: Option<&str>,
) -> bool {
    matches!(target, PlaybackTarget::Browser)
        && matches!(
            resolve_effective_playback_format(output_format, legacy_stream_extension),
            Ok(PlaybackStreamFormat::Ts)
        )
}

async fn finalize_playback_source(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    profile_id: Uuid,
    target: PlaybackTarget,
    raw_playback_mode: &str,
    title: &str,
    upstream_url: String,
    live: bool,
    catchup: bool,
    format: PlaybackStreamFormat,
    expires_at: Option<DateTime<Utc>>,
    browser_hls_preflight_required: bool,
) -> Result<PlaybackSourceResponse, AppError> {
    if browser_hls_preflight_required {
        match xtreme::probe_hls_playlist_url(&state.provider_http_client, &upstream_url).await {
            Ok(true) => {}
            Ok(false) => {
                warn!(title = %title, upstream_url = %upstream_url, "browser HLS preflight failed");
                return Ok(unsupported_playback(title, BROWSER_HLS_UNSUPPORTED_REASON));
            }
            Err(error) => {
                warn!(
                    title = %title,
                    upstream_url = %upstream_url,
                    error = ?error,
                    "browser HLS preflight errored"
                );
                return Ok(unsupported_playback(title, BROWSER_HLS_UNSUPPORTED_REASON));
            }
        }
    }

    playback_source_for_mode(
        state,
        headers,
        user_id,
        profile_id,
        target,
        raw_playback_mode,
        title,
        upstream_url,
        live,
        catchup,
        format,
        expires_at,
    )
}

#[cfg(test)]
mod tests {
    use super::relay_tokens::decode_relay_token;
    use super::resolve::{PlaybackStreamFormat, playback_source_for_mode};
    use super::*;

    fn sample_app_state() -> AppState {
        sample_app_state_with_public_origin(Some("https://app.example.com"))
    }

    fn sample_app_state_without_public_origin() -> AppState {
        sample_app_state_with_public_origin(None)
    }

    fn sample_app_state_with_public_origin(public_origin: Option<&str>) -> AppState {
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
                public_origin: public_origin
                    .map(|origin| Url::parse(origin).expect("public origin")),
                allowed_origins: public_origin
                    .map(|origin| vec![origin.to_string()])
                    .unwrap_or_default(),
                browser_cookie_secure: public_origin.is_some(),
                vpn_enabled: false,
                vpn_provider_name: None,
                meilisearch_url: None,
                meilisearch_api_key: None,
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
            receiver_channels: Arc::new(DashMap::new()),
        }
    }

    fn extract_relay_token(url: &str) -> String {
        Url::parse(url)
            .expect("relay url")
            .query_pairs()
            .find_map(|(key, value)| (key == "token").then(|| value.into_owned()))
            .expect("token query parameter")
    }

    fn local_request_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:8080"));
        headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static("http"),
        );
        headers
    }

    #[tokio::test]
    async fn playback_source_for_mode_keeps_direct_urls_in_direct_mode() {
        let state = sample_app_state();
        let response = playback_source_for_mode(
            &state,
            &HeaderMap::new(),
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            PlaybackTarget::Browser,
            "direct",
            "Arena 1",
            "https://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("direct playback source");

        assert_eq!(response.kind, "hls");
        assert_eq!(response.url, "https://provider.example.com/live/42.m3u8");
        assert!(response.expires_at.is_none());
    }

    #[tokio::test]
    async fn playback_source_for_mode_issues_signed_relay_urls() {
        let state = sample_app_state();
        let response = playback_source_for_mode(
            &state,
            &HeaderMap::new(),
            Uuid::from_u128(3),
            Uuid::from_u128(4),
            PlaybackTarget::Browser,
            "relay",
            "Arena 1",
            "https://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("relay playback source");

        assert_eq!(response.kind, "hls");
        assert!(
            response
                .url
                .starts_with("https://app.example.com/api/relay/hls?token=")
        );
        assert!(response.expires_at.is_some());

        let relay = decode_relay_token(
            &state.config,
            &extract_relay_token(&response.url),
            RelayAssetKind::Hls,
        )
        .expect("decode relay token");
        assert_eq!(relay.user_id, Uuid::from_u128(3));
        assert_eq!(relay.profile_id, Uuid::from_u128(4));
        assert_eq!(
            relay.upstream_url.as_str(),
            "https://provider.example.com/live/42.m3u8"
        );
    }

    #[tokio::test]
    async fn playback_source_for_mode_forces_relay_for_http_streams_on_https_pages() {
        let state = sample_app_state();
        let response = playback_source_for_mode(
            &state,
            &HeaderMap::new(),
            Uuid::from_u128(31),
            Uuid::from_u128(32),
            PlaybackTarget::Browser,
            "direct",
            "Arena 1",
            "http://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("forced relay playback source");

        assert_eq!(response.kind, "hls");
        assert!(
            response
                .url
                .starts_with("https://app.example.com/api/relay/hls?token=")
        );
        assert!(response.expires_at.is_some());

        let relay = decode_relay_token(
            &state.config,
            &extract_relay_token(&response.url),
            RelayAssetKind::Hls,
        )
        .expect("decode relay token");
        assert_eq!(
            relay.upstream_url.as_str(),
            "http://provider.example.com/live/42.m3u8"
        );
    }

    #[tokio::test]
    async fn playback_source_for_mode_keeps_http_streams_direct_on_http_pages() {
        let state = sample_app_state_without_public_origin();
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_static("127.0.0.1:8080"));
        headers.insert(
            HeaderName::from_static("x-forwarded-proto"),
            HeaderValue::from_static("http"),
        );

        let response = playback_source_for_mode(
            &state,
            &headers,
            Uuid::from_u128(33),
            Uuid::from_u128(34),
            PlaybackTarget::Browser,
            "direct",
            "Arena 1",
            "http://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("direct playback source");

        assert_eq!(response.kind, "hls");
        assert_eq!(response.url, "http://provider.example.com/live/42.m3u8");
        assert!(response.expires_at.is_none());
    }

    #[tokio::test]
    async fn playback_source_for_mode_bypasses_relay_in_local_dev() {
        let state = sample_app_state_without_public_origin();
        let headers = local_request_headers();

        let response = playback_source_for_mode(
            &state,
            &headers,
            Uuid::from_u128(43),
            Uuid::from_u128(44),
            PlaybackTarget::Browser,
            "relay",
            "Arena 1",
            "https://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("local dev playback source");

        assert_eq!(response.kind, "hls");
        assert_eq!(response.url, "https://provider.example.com/live/42.m3u8");
        assert!(response.expires_at.is_none());
    }

    #[tokio::test]
    async fn playback_source_for_mode_keeps_http_streams_direct_for_receivers() {
        let state = sample_app_state();
        let response = playback_source_for_mode(
            &state,
            &HeaderMap::new(),
            Uuid::from_u128(35),
            Uuid::from_u128(36),
            PlaybackTarget::ReceiverWeb,
            "direct",
            "Arena 1",
            "http://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("direct receiver playback source");

        assert_eq!(response.kind, "hls");
        assert_eq!(response.url, "http://provider.example.com/live/42.m3u8");
        assert!(response.expires_at.is_none());
    }

    #[tokio::test]
    async fn playback_source_for_mode_forces_relay_for_android_tv_receivers() {
        let state = sample_app_state();
        let response = playback_source_for_mode(
            &state,
            &HeaderMap::new(),
            Uuid::from_u128(45),
            Uuid::from_u128(46),
            PlaybackTarget::ReceiverAndroidTv,
            "direct",
            "Arena 1",
            "https://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        )
        .expect("android tv playback source");

        assert!(
            response
                .url
                .starts_with("https://app.example.com/api/relay/hls?token=")
        );
        assert!(response.expires_at.is_some());
    }

    #[test]
    fn browser_targets_require_hls_preflight_for_ts_streams() {
        assert!(target_requires_browser_hls_preflight(
            PlaybackTarget::Browser,
            "m3u8",
            Some("ts"),
        ));
        assert!(target_requires_browser_hls_preflight(
            PlaybackTarget::Browser,
            "ts",
            None,
        ));
        assert!(!target_requires_browser_hls_preflight(
            PlaybackTarget::Browser,
            "m3u8",
            Some("m3u8"),
        ));
        assert!(!target_requires_browser_hls_preflight(
            PlaybackTarget::ReceiverWeb,
            "ts",
            None,
        ));
    }

    #[tokio::test]
    async fn finalize_playback_source_returns_unsupported_when_browser_hls_preflight_fails() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/stream.m3u8",
                get(|| async move {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "video/mp2t")
                        .body(Body::from("not a playlist"))
                        .expect("response")
                }),
            );

            axum::serve(listener, app).await.expect("serve test app");
        });

        let state = sample_app_state_without_public_origin();
        let response = finalize_playback_source(
            &state,
            &local_request_headers(),
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            PlaybackTarget::Browser,
            "direct",
            "Arena 1",
            format!("http://{addr}/stream.m3u8"),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
            true,
        )
        .await
        .expect("playback response");

        assert_eq!(response.kind, "unsupported");
        assert_eq!(
            response.unsupported_reason.as_deref(),
            Some(BROWSER_HLS_UNSUPPORTED_REASON)
        );

        server.abort();
    }

    #[tokio::test]
    async fn finalize_playback_source_returns_hls_when_browser_hls_preflight_succeeds() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let server = tokio::spawn(async move {
            let app = Router::new().route(
                "/stream.m3u8",
                get(|| async move {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(
                            header::CONTENT_TYPE,
                            "application/vnd.apple.mpegurl",
                        )
                        .body(Body::from("#EXTM3U\n#EXT-X-VERSION:3\n"))
                        .expect("response")
                }),
            );

            axum::serve(listener, app).await.expect("serve test app");
        });

        let state = sample_app_state_without_public_origin();
        let response = finalize_playback_source(
            &state,
            &local_request_headers(),
            Uuid::from_u128(11),
            Uuid::from_u128(12),
            PlaybackTarget::Browser,
            "direct",
            "Arena 1",
            format!("http://{addr}/stream.m3u8"),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
            true,
        )
        .await
        .expect("playback response");

        assert_eq!(response.kind, "hls");
        assert_eq!(response.url, format!("http://{addr}/stream.m3u8"));

        server.abort();
    }
}
