use super::super::{request_base_url, rewrite_channel_logo_url};
use super::*;

#[test]
fn parses_guide_category_pagination_defaults_and_caps_limit() {
    let (offset, limit) = parse_guide_category_pagination(GuideCategoryQuery {
        offset: None,
        limit: Some(GUIDE_MAX_LIMIT + 25),
        with_epg_only: None,
        quality_channels_only: None,
    })
    .expect("pagination");

    assert_eq!(offset, 0);
    assert_eq!(limit, GUIDE_MAX_LIMIT);
}

#[test]
fn rejects_negative_guide_category_offset() {
    let error = parse_guide_category_pagination(GuideCategoryQuery {
        offset: Some(-1),
        limit: Some(10),
        with_epg_only: None,
        quality_channels_only: None,
    })
    .expect_err("negative offset should fail");

    match error {
        AppError::BadRequest(message) => assert!(message.contains("offset")),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn computes_next_guide_offset_only_when_more_results_exist() {
    assert_eq!(next_guide_offset(0, 40, 81), Some(40));
    assert_eq!(next_guide_offset(40, 40, 80), None);
    assert_eq!(next_guide_offset(80, 40, 80), None);
}

#[test]
fn guide_preferences_normalization_deduplicates_and_trims() {
    let normalized = normalize_category_ids(vec![
        " sports ".to_string(),
        "sports".to_string(),
        "".to_string(),
        "news".to_string(),
        "news".to_string(),
    ]);

    assert_eq!(normalized, vec!["sports".to_string(), "news".to_string()]);
}

#[test]
fn guide_preferences_normalization_preserves_empty_arrays() {
    let normalized = normalize_category_ids(Vec::new());

    assert!(normalized.is_empty());
}

#[test]
fn maps_guide_category_summary_favorite_state() {
    let summary = map_guide_category_summary(GuideCategorySummaryRow {
        id: "sports".to_string(),
        name: "Sports".to_string(),
        channel_count: 12,
        live_now_count: 3,
        is_favorite: true,
    });

    assert_eq!(summary.id, "sports");
    assert_eq!(summary.name, "Sports");
    assert_eq!(summary.channel_count, 12);
    assert_eq!(summary.live_now_count, 3);
    assert!(summary.is_favorite);
}

#[tokio::test]
async fn expired_visibility_cache_entries_are_removed_without_retaining_guard() {
    let state = sample_app_state();
    let cache_key = (Uuid::from_u128(61), None);
    state.channel_visibility_cache.insert(
        cache_key,
        CachedChannelVisibilityMap {
            values: Arc::new(HashMap::from([(
                Uuid::from_u128(62),
                ChannelVisibility {
                    is_hidden: false,
                    is_placeholder: false,
                },
            )])),
            expires_at: Instant::now() - Duration::from_secs(1),
        },
    );

    let cached = cached_channel_visibility_map(&state, cache_key, Instant::now());

    assert!(cached.is_none());
    assert!(!state.channel_visibility_cache.contains_key(&cache_key));
}

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
async fn maps_guide_entry_rows_into_nested_payloads() {
    let now = Utc::now();
    let state = sample_app_state();
    let request_base_url = Url::parse("https://app.example.com").expect("request base url");
    let entry = map_guide_category_entry(
        &state,
        &request_base_url,
        Uuid::from_u128(51),
        GuideCategoryEntryRow {
            channel_id: Uuid::nil(),
            profile_id: Uuid::from_u128(52),
            channel_name: "Arena 1".to_string(),
            logo_url: Some("https://example.com/logo.png".to_string()),
            category_name: Some("Uncategorized".to_string()),
            remote_stream_id: 7,
            epg_channel_id: Some("arena.1".to_string()),
            has_catchup: true,
            archive_duration_hours: Some(48),
            stream_extension: Some("m3u8".to_string()),
            is_favorite: true,
            is_ppv: false,
            is_ppv_favorite: false,
            program_id: Some(Uuid::from_u128(42)),
            program_channel_id: Some(Uuid::nil()),
            program_channel_name: Some("Arena 1".to_string()),
            program_title: Some("Matchday Live".to_string()),
            program_description: Some("Quarterfinal".to_string()),
            program_start_at: Some(now),
            program_end_at: Some(now + ChronoDuration::hours(2)),
            program_can_catchup: Some(true),
        },
    )
    .expect("guide entry");

    assert_eq!(entry.channel.name, "Arena 1");
    assert_eq!(
        entry.channel.category_name.as_deref(),
        Some("Uncategorized")
    );
    assert!(entry.channel.is_favorite);
    assert_eq!(
        entry.program.as_ref().map(|program| program.title.as_str()),
        Some("Matchday Live")
    );
    assert_eq!(
        entry
            .program
            .as_ref()
            .and_then(|program| program.channel_name.as_deref()),
        Some("Arena 1")
    );
    assert_eq!(
        entry.program.as_ref().map(|program| program.can_catchup),
        Some(true)
    );
}

#[tokio::test]
async fn maps_guide_entry_rows_without_programs() {
    let state = sample_app_state();
    let request_base_url = Url::parse("https://app.example.com").expect("request base url");
    let entry = map_guide_category_entry(
        &state,
        &request_base_url,
        Uuid::from_u128(53),
        GuideCategoryEntryRow {
            channel_id: Uuid::nil(),
            profile_id: Uuid::from_u128(54),
            channel_name: "Arena 2".to_string(),
            logo_url: None,
            category_name: Some("Sports".to_string()),
            remote_stream_id: 8,
            epg_channel_id: None,
            has_catchup: false,
            archive_duration_hours: None,
            stream_extension: Some("m3u8".to_string()),
            is_favorite: false,
            is_ppv: false,
            is_ppv_favorite: false,
            program_id: None,
            program_channel_id: None,
            program_channel_name: None,
            program_title: None,
            program_description: None,
            program_start_at: None,
            program_end_at: None,
            program_can_catchup: None,
        },
    )
    .expect("guide entry");

    assert_eq!(entry.channel.name, "Arena 2");
    assert!(entry.program.is_none());
}

#[tokio::test]
async fn maps_guide_entry_rows_with_incomplete_programs_without_panicking() {
    let state = sample_app_state();
    let request_base_url = Url::parse("https://app.example.com").expect("request base url");
    let entry = map_guide_category_entry(
        &state,
        &request_base_url,
        Uuid::from_u128(55),
        GuideCategoryEntryRow {
            channel_id: Uuid::nil(),
            profile_id: Uuid::from_u128(56),
            channel_name: "Arena 3".to_string(),
            logo_url: None,
            category_name: Some("Sports".to_string()),
            remote_stream_id: 9,
            epg_channel_id: None,
            has_catchup: false,
            archive_duration_hours: None,
            stream_extension: Some("m3u8".to_string()),
            is_favorite: false,
            is_ppv: false,
            is_ppv_favorite: false,
            program_id: Some(Uuid::from_u128(57)),
            program_channel_id: Some(Uuid::nil()),
            program_channel_name: Some("Arena 3".to_string()),
            program_title: Some("Broken Listing".to_string()),
            program_description: None,
            program_start_at: None,
            program_end_at: Some(Utc::now() + ChronoDuration::hours(1)),
            program_can_catchup: Some(false),
        },
    )
    .expect("guide entry");

    assert_eq!(entry.channel.name, "Arena 3");
    assert!(entry.program.is_none());
}

#[tokio::test]
async fn rewrite_channel_logo_url_relays_http_logos_on_https_pages() {
    let state = sample_app_state();
    let request_base_url = Url::parse("https://app.example.com").expect("request base url");

    let logo_url = rewrite_channel_logo_url(
        &state,
        &request_base_url,
        Uuid::from_u128(41),
        Uuid::from_u128(42),
        Some("http://provider.example.com/logo.png".to_string()),
    )
    .expect("rewritten logo url")
    .expect("logo url");

    assert!(logo_url.starts_with("https://app.example.com/api/relay/asset?token="));
}

#[tokio::test]
async fn rewrite_channel_logo_url_keeps_https_logos_direct() {
    let state = sample_app_state();
    let request_base_url = Url::parse("https://app.example.com").expect("request base url");

    let logo_url = rewrite_channel_logo_url(
        &state,
        &request_base_url,
        Uuid::from_u128(43),
        Uuid::from_u128(44),
        Some("https://provider.example.com/logo.png".to_string()),
    )
    .expect("rewritten logo url");

    assert_eq!(
        logo_url.as_deref(),
        Some("https://provider.example.com/logo.png")
    );
}

#[tokio::test]
async fn request_base_url_prefers_public_origin_over_forwarded_headers() {
    let state = sample_app_state();
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-forwarded-host"),
        HeaderValue::from_static("internal.example.com"),
    );
    headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static("http"),
    );

    let url = request_base_url(&state.config, &headers).expect("request base url");

    assert_eq!(url.as_str(), "https://app.example.com/");
}
