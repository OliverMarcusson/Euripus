use super::*;

#[test]
fn builds_catchup_urls() {
    let credentials = XtreamCredentials {
        base_url: "https://iptv.example.com".to_string(),
        username: "user".to_string(),
        password: "pass".to_string(),
        output_format: "m3u8".to_string(),
    };

    let url = build_catchup_url(
        &credentials,
        7,
        Some("m3u8"),
        DateTime::parse_from_rfc3339("2026-04-04T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
        DateTime::parse_from_rfc3339("2026-04-04T13:00:00Z")
            .unwrap()
            .with_timezone(&Utc),
    )
    .expect("catchup url");

    assert!(url.contains("timeshift/user/pass/60/2026-04-04:12-00/7.m3u8"));
}

#[test]
fn builds_on_demand_urls() {
    let credentials = XtreamCredentials {
        base_url: "https://iptv.example.com".to_string(),
        username: "user".to_string(),
        password: "pass".to_string(),
        output_format: "m3u8".to_string(),
    };

    assert!(
        build_on_demand_stream_url(&credentials, "movie", "42", Some("mp4"))
            .expect("movie url")
            .contains("movie/user/pass/42.mp4")
    );
    assert!(
        build_on_demand_stream_url(&credentials, "series", "84", Some("mkv"))
            .expect("episode url")
            .contains("series/user/pass/84.mkv")
    );
}

#[test]
fn decodes_series_payload_with_string_ids() {
    let payload: serde_json::Value = serde_json::from_str(r#"{
          "series_id":"12","name":"Example","category_id":7,"rating":"8.5",
          "last_modified":"1710000000","episode_run_time":"45","backdrop_path":["https://img/back.jpg"]
        }"#).expect("series payload");
    assert_eq!(object_string(&payload, "series_id").as_deref(), Some("12"));
    assert_eq!(value_to_f64(payload.get("rating")), Some(8.5));
    assert_eq!(
        payload.get("episode_run_time").and_then(value_to_i32),
        Some(45)
    );
}

#[test]
fn parses_provider_durations() {
    assert_eq!(parse_duration_minutes("01:42:30"), Some(102));
    assert_eq!(parse_duration_minutes("42:30"), Some(42));
    assert_eq!(parse_duration_minutes("unknown"), None);
}

#[test]
fn canonicalizes_hls_identity_without_query_or_fragment() {
    assert_eq!(
        canonical_hls_stream_identity("https://cdn.example.com/no-event/idle.m3u8?token=secret#x"),
        Some((
            "https://cdn.example.com".to_string(),
            "sha256:f859a2cc2e26ebcc6d69014956aa9f71cd97383c42824d8fc3675569e2818121".to_string(),
        ))
    );
    assert_eq!(canonical_hls_stream_identity("file:///tmp/a.m3u8"), None);
}

#[test]
fn detects_hls_playlists_from_manifest_markers() {
    assert!(looks_like_hls_playlist("#EXTM3U\n#EXT-X-VERSION:3\n"));
    assert!(looks_like_hls_playlist("#EXTINF:6,\nsegment001.ts\n"));
    assert!(!looks_like_hls_playlist("not a playlist"));
}
