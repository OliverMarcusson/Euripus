use super::*;

#[test]
fn encrypts_and_decrypts_provider_secrets() {
    let key = *b"0123456789abcdef0123456789abcdef";
    let encrypted = encrypt_secret(&key, "super-secret").expect("encrypt");
    let decrypted = decrypt_secret(&key, &encrypted).expect("decrypt");
    assert_eq!(decrypted, "super-secret");
}

#[test]
fn hashes_refresh_tokens_deterministically() {
    let first = hash_refresh_token("same-token");
    let second = hash_refresh_token("same-token");
    assert_eq!(first, second);
    assert_eq!(first.len(), 64);
}

#[test]
fn classify_channel_visibility_hides_placeholder_ppv_channels() {
    let visibility = classify_channel_visibility(
        "ENDED | GOLF MAJOR ON THE RANGE | Wed 08 Apr 15:00 CEST (SE) | 8K EXCLUSIVE | SE: VIAPLAY PPV 2",
        Some("SE| VIAPLAY PPV"),
        &["Golf Major On The Range".to_string()],
    );

    assert!(visibility.is_hidden);
    assert!(visibility.is_placeholder);
}

#[test]
fn classify_channel_visibility_hides_generic_numbered_ppv_channels_without_events() {
    let visibility = classify_channel_visibility(":Viaplay SE 13", Some("SE| VIAPLAY PPV"), &[]);

    assert!(visibility.is_hidden);
    assert!(!visibility.is_placeholder);
}

#[test]
fn classify_channel_visibility_keeps_event_specific_ppv_channels_visible() {
    let visibility = classify_channel_visibility(
        "SE: VIAPLAY PPV 5",
        Some("SE| VIAPLAY PPV"),
        &["Golf Major Par 3 Contest".to_string()],
    );

    assert!(!visibility.is_hidden);
    assert!(!visibility.is_placeholder);
}

#[test]
fn classify_channel_visibility_keeps_live_prefixed_ppv_channels_visible() {
    let visibility = classify_channel_visibility(
        "LIVE | GOLF MAJOR PAR 3 CONTEST | SE: VIAPLAY PPV 5",
        Some("SE| VIAPLAY PPV"),
        &[],
    );

    assert!(!visibility.is_hidden);
    assert!(!visibility.is_placeholder);
}

#[test]
fn classify_channel_visibility_keeps_next_prefixed_ppv_channels_visible() {
    let visibility = classify_channel_visibility(
        "NEXT | NHL ON THE FLY | SE: VIAPLAY PPV 1",
        Some("SE| VIAPLAY PPV"),
        &[],
    );

    assert!(!visibility.is_hidden);
    assert!(!visibility.is_placeholder);
}

#[test]
fn classify_channel_visibility_hides_ppv_channels_with_past_month_day_marker() {
    let visibility = classify_channel_visibility_at(
        "PSG vs Liverpool @ Apr 8 20:55 : TeliaPlay SE 26",
        Some("SE| PLAY+ PPV VIP"),
        &[],
        NaiveDate::from_ymd_opt(2026, 4, 9).expect("valid date"),
    );

    assert!(visibility.is_hidden);
    assert!(!visibility.is_placeholder);
}

#[test]
fn classify_channel_visibility_keeps_same_day_ppv_channels_visible() {
    let visibility = classify_channel_visibility_at(
        "PSG vs Liverpool @ Apr 9 20:55 : TeliaPlay SE 26",
        Some("SE| PLAY+ PPV VIP"),
        &[],
        NaiveDate::from_ymd_opt(2026, 4, 9).expect("valid date"),
    );

    assert!(!visibility.is_hidden);
    assert!(!visibility.is_placeholder);
}

#[tokio::test]
async fn json_response_with_revalidation_returns_json_and_cache_headers() {
    let payload = serde_json::json!({
        "status": "ok",
        "items": [1, 2, 3],
    });

    let response =
        json_response_with_revalidation(&HeaderMap::new(), &payload).expect("cached json response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok()),
        Some("private, no-cache")
    );
    assert!(
        response.headers().contains_key(header::ETAG),
        "etag header should be present"
    );

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&body).expect("json body"),
        payload
    );
}

#[tokio::test]
async fn json_response_with_revalidation_returns_not_modified_for_matching_etag() {
    let payload = serde_json::json!({
        "status": "ok",
    });
    let initial =
        json_response_with_revalidation(&HeaderMap::new(), &payload).expect("initial response");
    let etag = initial
        .headers()
        .get(header::ETAG)
        .and_then(|value| value.to_str().ok())
        .expect("etag header")
        .to_string();
    let mut headers = HeaderMap::new();
    headers.insert(
        header::IF_NONE_MATCH,
        HeaderValue::from_str(&etag).expect("etag header value"),
    );

    let response =
        json_response_with_revalidation(&headers, &payload).expect("not modified response");

    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body");
    assert!(body.is_empty());
}
