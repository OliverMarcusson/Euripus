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
            cast_transcoding_enabled: false,
            cast_transcode_encoder: "h264_nvenc".to_string(),
            cast_transcode_directory: "/tmp/euripus-test-transcodes".to_string(),
            daily_sync_hour_local: 6,
            public_origin: public_origin.map(|origin| Url::parse(origin).expect("public origin")),
            allowed_origins: public_origin
                .map(|origin| vec![origin.to_string()])
                .unwrap_or_default(),
            browser_cookie_secure: public_origin.is_some(),
            vpn_enabled: false,
            vpn_provider_name: None,
            openrouter_api_key: None,
            openrouter_model: "openai/gpt-4.1-mini".to_string(),
            sports_api_base_url: None,
            google_client_id: None,
            google_client_secret: None,
            google_calendar_redirect_url: None,
            admin_password: None,
            pi_executable: "pi".to_string(),
            pi_model: "gpt-5.6-terra".to_string(),
        }),
        provider_http_client: reqwest::Client::new(),
        relay_http_client: reqwest::Client::new(),
        user_database_locks: Arc::new(DashMap::new()),
        session_cache: Arc::new(DashMap::new()),
        relay_profile_cache: Arc::new(DashMap::new()),
        channel_visibility_cache: Arc::new(DashMap::new()),
        receiver_channels: Arc::new(DashMap::new()),
        cast_transcodes: Arc::new(Mutex::new(transcode::CastTranscodeManager::default())),
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
        None,
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
        None,
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
        None,
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
        None,
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
async fn playback_source_for_mode_uses_relay_in_local_dev() {
    let state = sample_app_state_without_public_origin();
    let headers = local_request_headers();

    let response = playback_source_for_mode(
        &state,
        &headers,
        None,
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
    assert!(
        response
            .url
            .starts_with("http://127.0.0.1:8080/api/relay/hls?token=")
    );
    assert!(response.expires_at.is_some());
}

#[tokio::test]
async fn playback_source_for_mode_keeps_http_streams_direct_for_receivers() {
    let state = sample_app_state();
    let response = playback_source_for_mode(
        &state,
        &HeaderMap::new(),
        None,
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
        None,
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
fn browser_targets_require_hls_preflight_for_non_hls_streams() {
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
    assert!(target_requires_browser_hls_preflight(
        PlaybackTarget::Browser,
        "m3u8",
        Some("mp4"),
    ));
    assert!(target_requires_browser_hls_preflight(
        PlaybackTarget::Browser,
        "legacy",
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
        None,
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
                    .header(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")
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
        None,
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

#[test]
fn google_cast_receiver_uses_cast_playback_target() {
    assert_eq!(
        playback_target_for_receiver_app("receiver-google-cast"),
        PlaybackTarget::Cast
    );
}

#[tokio::test]
async fn playback_source_for_mode_always_relays_cast_targets() {
    let state = sample_app_state();
    let response = playback_source_for_mode(
        &state,
        &local_request_headers(),
        None,
        Uuid::from_u128(41),
        Uuid::from_u128(42),
        PlaybackTarget::Cast,
        "direct",
        "Arena 1",
        "https://provider.example.com/live/42.m3u8".to_string(),
        true,
        false,
        PlaybackStreamFormat::Hls,
        None,
    )
    .expect("cast playback source");

    assert!(
        response
            .url
            .starts_with("https://app.example.com/api/relay/hls?token=")
    );
}

#[tokio::test]
async fn playback_source_for_mode_prefers_receiver_public_origin_for_receiver_targets() {
    let state = sample_app_state_without_public_origin();
    let headers = local_request_headers();

    let response = playback_source_for_mode(
        &state,
        &headers,
        Some("http://192.168.0.67:5173"),
        Uuid::from_u128(51),
        Uuid::from_u128(52),
        PlaybackTarget::ReceiverAndroidTv,
        "direct",
        "Arena 1",
        "https://provider.example.com/live/42.m3u8".to_string(),
        true,
        false,
        PlaybackStreamFormat::Hls,
        None,
    )
    .expect("receiver playback source");

    assert!(
        response
            .url
            .starts_with("http://192.168.0.67:5173/api/relay/hls?token=")
    );
}
