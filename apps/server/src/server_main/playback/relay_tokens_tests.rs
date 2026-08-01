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
            cast_transcoding_enabled: false,
            cast_transcode_encoder: "h264_nvenc".to_string(),
            cast_transcode_directory: "/tmp/euripus-test-transcodes".to_string(),
            daily_sync_hour_local: 6,
            public_origin: Some(Url::parse("https://app.example.com").expect("public origin")),
            allowed_origins: vec!["https://app.example.com".to_string()],
            browser_cookie_secure: true,
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
            tmdb_api_key: None,
            tmdb_countries: Vec::new(),
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
