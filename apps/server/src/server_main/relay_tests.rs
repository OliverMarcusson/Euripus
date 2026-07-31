use super::super::playback::relay_tokens::decode_relay_token;
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

fn extract_relay_urls_from_line(line: &str) -> Vec<String> {
    line.split('"')
        .filter(|segment| segment.starts_with("https://app.example.com/api/relay/"))
        .map(ToString::to_string)
        .collect()
}

#[tokio::test]
async fn rewrite_hls_manifest_rewrites_variant_and_segment_uris() {
    let state = sample_app_state();
    let user_id = Uuid::from_u128(7);
    let profile_id = Uuid::from_u128(8);
    let expires_at = Utc::now() + ChronoDuration::minutes(10);
    let public_base_url = Url::parse("https://app.example.com").expect("public url");
    let upstream_base_url =
        Url::parse("https://provider.example.com/live/master.m3u8").expect("upstream url");
    let manifest = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",NAME=\"English\",URI=\"audio/en.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=3000000\nvideo/main.m3u8?token=abc\n#EXTINF:6.0,\nsegment001.ts\n";

    let rewritten = rewrite_hls_manifest(
        &state,
        user_id,
        profile_id,
        expires_at,
        &public_base_url,
        &upstream_base_url,
        manifest,
    )
    .expect("rewrite manifest");

    assert!(rewritten.contains("https://app.example.com/api/relay/hls?token="));
    assert!(rewritten.contains("https://app.example.com/api/relay/raw?token="));

    let urls = rewritten
        .lines()
        .filter(|line| line.contains("/api/relay/"))
        .flat_map(extract_relay_urls_from_line)
        .map(|url| {
            let kind = if url.contains("/api/relay/hls") {
                RelayAssetKind::Hls
            } else {
                RelayAssetKind::Raw
            };
            decode_relay_token(&state.config, &extract_relay_token(&url), kind)
        })
        .collect::<Vec<_>>();

    assert_eq!(urls.len(), 3);
    assert!(urls.iter().all(Result::is_ok));
}

#[test]
fn detects_known_provider_placeholder_manifests() {
    let base = Url::parse("https://provider.example.com/live/channel.m3u8").expect("upstream url");

    assert!(is_provider_placeholder_manifest(
        &base,
        "#EXTM3U\n#EXTINF:15,\nblack.ts?token=ignored\n#EXT-X-ENDLIST\n",
    ));
    assert!(is_provider_placeholder_manifest(
        &base,
        "#EXTM3U\n#EXTINF:15,\nhttps://cdn.example.com/video/BLACK.TS?x=1\n#EXT-X-ENDLIST\n",
    ));
}

#[test]
fn rejects_non_placeholder_hls_manifests() {
    let base = Url::parse("https://provider.example.com/live/channel.m3u8").expect("upstream url");
    let cases = [
        // A master playlist is never a media placeholder.
        "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nblack.ts\n#EXT-X-ENDLIST\n",
        // Multiple media segments are not the strict placeholder shape.
        "#EXTM3U\n#EXTINF:5,\nblack.ts\n#EXTINF:5,\nblack.ts\n#EXT-X-ENDLIST\n",
        // A live playlist without ENDLIST may legitimately use a black slate.
        "#EXTM3U\n#EXTINF:5,\nblack.ts\n",
        // An ordinary one-segment recording is valid media.
        "#EXTM3U\n#EXTINF:5,\nrecording.ts\n#EXT-X-ENDLIST\n",
        // A lookalike without the required HLS header is not a manifest.
        "#EXTINF:5,\nblack.ts\n#EXT-X-ENDLIST\n",
        // A URI without segment duration metadata is not the known placeholder.
        "#EXTM3U\nblack.ts\n#EXT-X-ENDLIST\n",
        // ENDLIST before the segment URI does not pair EXTINF with that URI.
        "#EXTM3U\n#EXTINF:5,\n#EXT-X-ENDLIST\nblack.ts\n",
    ];

    for manifest in cases {
        assert!(!is_provider_placeholder_manifest(&base, manifest));
    }
}

#[test]
fn placeholder_response_has_terminal_machine_readable_status() {
    let response = provider_placeholder_response().expect("placeholder response");

    assert_eq!(response.status().as_u16(), PROVIDER_PLACEHOLDER_STATUS);
    assert_eq!(
        response
            .headers()
            .get("x-euripus-playback-error")
            .and_then(|value| value.to_str().ok()),
        Some(PROVIDER_PLACEHOLDER_ERROR)
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("no-store")
    );
}

#[test]
fn relay_upstream_request_forwards_selected_headers() {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=100-"));
    headers.insert(
        HeaderName::from_static("if-range"),
        HeaderValue::from_static("\"etag-1\""),
    );
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("EuripusTest/1.0"),
    );

    let request = relay_upstream_request(
        &client,
        Url::parse("https://provider.example.com/video.ts").expect("upstream url"),
        &headers,
        &["range", "if-range", "user-agent"],
    )
    .build()
    .expect("relay request");

    assert_eq!(
        request
            .headers()
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok()),
        Some("bytes=100-")
    );
    assert_eq!(
        request
            .headers()
            .get("if-range")
            .and_then(|value| value.to_str().ok()),
        Some("\"etag-1\"")
    );
    assert_eq!(
        request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some("EuripusTest/1.0")
    );
}

#[test]
fn relay_upstream_request_sets_default_user_agent_when_missing() {
    let client = reqwest::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=100-"));

    let request = relay_upstream_request(
        &client,
        Url::parse("https://provider.example.com/video.ts").expect("upstream url"),
        &headers,
        &["range", "if-range", "user-agent"],
    )
    .build()
    .expect("relay request");

    assert_eq!(
        request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some("EuripusRelay/1.0")
    );
}

#[tokio::test]
async fn relay_stream_response_preserves_partial_content_and_headers() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind listener");
    let addr = listener.local_addr().expect("listener addr");
    let server = tokio::spawn(async move {
        let app = Router::new().route(
            "/video.ts",
            get(|headers: HeaderMap| async move {
                let range = headers
                    .get(header::RANGE)
                    .and_then(|value| value.to_str().ok());
                let if_range = headers
                    .get("if-range")
                    .and_then(|value| value.to_str().ok());
                let user_agent = headers
                    .get(header::USER_AGENT)
                    .and_then(|value| value.to_str().ok());

                if range == Some("bytes=1-4")
                    && if_range == Some("\"etag-1\"")
                    && user_agent == Some("EuripusTest/1.0")
                {
                    Response::builder()
                        .status(StatusCode::PARTIAL_CONTENT)
                        .header(header::CONTENT_TYPE, "video/mp2t")
                        .header(header::CONTENT_LENGTH, "4")
                        .header(header::CONTENT_RANGE, "bytes 1-4/10")
                        .header(header::ACCEPT_RANGES, "bytes")
                        .header(header::ETAG, "\"etag-1\"")
                        .header(header::CACHE_CONTROL, "public, max-age=30")
                        .body(Body::from("data"))
                        .expect("partial content response")
                } else {
                    Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
                        .body(Body::from("missing headers"))
                        .expect("bad request response")
                }
            }),
        );

        axum::serve(listener, app)
            .await
            .expect("serve relay upstream");
    });

    let state = sample_app_state();
    let mut headers = HeaderMap::new();
    headers.insert(header::RANGE, HeaderValue::from_static("bytes=1-4"));
    headers.insert(
        HeaderName::from_static("if-range"),
        HeaderValue::from_static("\"etag-1\""),
    );
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("EuripusTest/1.0"),
    );

    let response = relay_stream_response(
        &state,
        Url::parse(&format!("http://{addr}/video.ts")).expect("upstream url"),
        &headers,
        "raw-stream",
        "test-relay-token",
    )
    .await
    .expect("relay response");

    assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_RANGE)
            .and_then(|value| value.to_str().ok()),
        Some("bytes 1-4/10")
    );
    assert_eq!(
        response
            .headers()
            .get(header::ACCEPT_RANGES)
            .and_then(|value| value.to_str().ok()),
        Some("bytes")
    );
    assert_eq!(
        response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok()),
        Some("\"etag-1\"")
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("public, max-age=30")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("relay body");
    assert_eq!(body.as_ref(), b"data");

    server.abort();
}
