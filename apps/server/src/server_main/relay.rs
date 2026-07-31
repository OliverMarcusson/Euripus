use super::playback::relay_tokens::{
    RelayTokenQuery, issue_relay_token, relay_asset_kind_for_url, relay_url_for_token,
    validate_relay_token,
};
use super::*;
use futures_util::StreamExt;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tracing::debug;

fn playback_diag_id(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest[..6]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>()
}

fn relay_upstream_host(url: &Url) -> String {
    url.host_str().unwrap_or("unknown").to_string()
}

fn relay_header_value(headers: &HeaderMap, name: HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

fn upstream_header_value(
    headers: &reqwest::header::HeaderMap,
    name: reqwest::header::HeaderName,
) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string)
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/relay/hls", get(relay_hls_playlist))
        .route("/relay/raw", get(relay_raw_stream))
        .route("/relay/asset", get(relay_asset))
}

async fn relay_hls_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RelayTokenQuery>,
) -> Result<Response, AppError> {
    let relay_token_id = playback_diag_id(&query.token);
    let started_at = Instant::now();
    let relay = match validate_relay_token(&state, &query.token, RelayAssetKind::Hls).await {
        Ok(relay) => relay,
        Err(error) => {
            warn!(
                component = "euripus-relay",
                asset = "hls-playlist",
                relay_token_id = %relay_token_id,
                "relay token validation failed before contacting the IPTV provider"
            );
            return Err(error);
        }
    };
    let public_base_url = request_base_url(&state.config, &headers)?;
    let upstream_host = relay_upstream_host(&relay.upstream_url);

    let response = relay_upstream_request(
        &state.relay_http_client,
        relay.upstream_url.clone(),
        &headers,
        &["user-agent"],
    )
    .send()
    .await
    .map_err(|error| {
        warn!(
            component = "iptv-provider",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            error = ?error,
            "upstream HLS playlist request failed before a response"
        );
        AppError::Internal(anyhow!(error))
    })?;
    let upstream_response_ms = started_at.elapsed().as_millis();
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    let response_url = response.url().clone();
    let upstream_headers = response.headers().clone();
    let content_type = upstream_headers
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/vnd.apple.mpegurl"));
    let bytes = response.bytes().await.map_err(|error| {
        warn!(
            component = "iptv-provider",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            error = ?error,
            "failed to read upstream HLS playlist body"
        );
        AppError::Internal(anyhow!(error))
    })?;
    let upstream_body_ms = started_at.elapsed().as_millis();
    if !status.is_success() {
        warn!(
            component = "iptv-provider",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            status = %status,
            upstream_response_ms,
            upstream_body_ms,
            response_bytes = bytes.len(),
            "IPTV provider returned a non-success HLS playlist response"
        );
        return relay_response_headers(
            Response::builder().status(status),
            &upstream_headers,
            &["content-type", "content-length", "cache-control"],
        )
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(anyhow!(error)));
    }

    if bytes.len() > RELAY_PLAYLIST_MAX_BYTES {
        warn!(
            component = "euripus-relay",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            bytes = bytes.len(),
            "upstream playlist exceeded the Euripus relay size limit"
        );
        return Err(AppError::BadRequest(
            "The upstream playlist exceeded the relay size limit.".to_string(),
        ));
    }

    let manifest = String::from_utf8(bytes.to_vec()).map_err(|_| {
        warn!(
            component = "iptv-provider",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            "upstream playlist body was not valid UTF-8"
        );
        AppError::BadRequest("The upstream playlist could not be decoded as UTF-8.".to_string())
    })?;
    if is_provider_placeholder_manifest(&response_url, &manifest) {
        warn!(
            component = "iptv-provider",
            event = "provider-placeholder-detected",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            "IPTV provider returned a known unavailable-channel placeholder"
        );
        return provider_placeholder_response();
    }

    let rewritten = rewrite_hls_manifest(
        &state,
        relay.user_id,
        relay.profile_id,
        relay.expires_at,
        &public_base_url,
        &response_url,
        &manifest,
    )
    .map_err(|error| {
        warn!(
            component = "euripus-relay",
            asset = "hls-playlist",
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            error = ?error,
            "failed to rewrite upstream HLS playlist into relay URLs"
        );
        error
    })?;
    debug!(
        component = "euripus-relay",
        asset = "hls-playlist",
        relay_token_id = %relay_token_id,
        upstream_host = %upstream_host,
        status = %status,
        upstream_response_ms,
        upstream_body_ms,
        total_relay_ms = started_at.elapsed().as_millis(),
        response_bytes = bytes.len(),
        upstream_content_type = ?upstream_header_value(&upstream_headers, reqwest::header::CONTENT_TYPE),
        upstream_cache_control = ?upstream_header_value(&upstream_headers, reqwest::header::CACHE_CONTROL),
        "rewrote upstream HLS playlist into relay URLs"
    );

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(rewritten))
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

async fn relay_raw_stream(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RelayTokenQuery>,
) -> Result<Response, AppError> {
    let relay_token_id = playback_diag_id(&query.token);
    let relay = match validate_relay_token(&state, &query.token, RelayAssetKind::Raw).await {
        Ok(relay) => relay,
        Err(error) => {
            warn!(
                component = "euripus-relay",
                asset = "raw-stream",
                relay_token_id = %relay_token_id,
                "relay token validation failed before contacting the IPTV provider"
            );
            return Err(error);
        }
    };

    relay_stream_response(
        &state,
        relay.upstream_url,
        &headers,
        "raw-stream",
        &relay_token_id,
    )
    .await
}

async fn relay_asset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RelayTokenQuery>,
) -> Result<Response, AppError> {
    let relay_token_id = playback_diag_id(&query.token);
    let relay = match validate_relay_token(&state, &query.token, RelayAssetKind::Asset).await {
        Ok(relay) => relay,
        Err(error) => {
            warn!(
                component = "euripus-relay",
                asset = "hls-asset",
                relay_token_id = %relay_token_id,
                "relay token validation failed before contacting the IPTV provider"
            );
            return Err(error);
        }
    };

    relay_stream_response(
        &state,
        relay.upstream_url,
        &headers,
        "hls-asset",
        &relay_token_id,
    )
    .await
}

pub(super) async fn relay_stream_response(
    state: &AppState,
    upstream_url: Url,
    headers: &HeaderMap,
    asset: &str,
    relay_token_id: &str,
) -> Result<Response, AppError> {
    let upstream_host = relay_upstream_host(&upstream_url);
    let started_at = Instant::now();
    let request_range = relay_header_value(headers, header::RANGE);
    let response = relay_upstream_request(
        &state.relay_http_client,
        upstream_url,
        headers,
        &["range", "if-range", "user-agent"],
    )
    .send()
    .await
    .map_err(|error| {
        warn!(
            component = "iptv-provider",
            asset = asset,
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            error = ?error,
            "upstream stream request failed before a response"
        );
        AppError::Internal(anyhow!(error))
    })?;
    let upstream_response_ms = started_at.elapsed().as_millis();
    let status = StatusCode::from_u16(response.status().as_u16())
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    let upstream_headers = response.headers().clone();
    let response_content_length =
        upstream_header_value(&upstream_headers, reqwest::header::CONTENT_LENGTH);
    let response_content_range =
        upstream_header_value(&upstream_headers, reqwest::header::CONTENT_RANGE);
    let response_content_type =
        upstream_header_value(&upstream_headers, reqwest::header::CONTENT_TYPE);
    if !status.is_success() {
        warn!(
            component = "iptv-provider",
            asset = asset,
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            status = %status,
            upstream_response_ms,
            request_range = ?request_range,
            response_content_length = ?response_content_length,
            response_content_range = ?response_content_range,
            "IPTV provider returned a non-success stream response"
        );
    } else {
        debug!(
            component = "euripus-relay",
            asset = asset,
            relay_token_id = %relay_token_id,
            upstream_host = %upstream_host,
            status = %status,
            upstream_response_ms,
            request_range = ?request_range,
            response_content_type = ?response_content_type,
            response_content_length = ?response_content_length,
            response_content_range = ?response_content_range,
            "received upstream stream response headers"
        );
    }
    let first_chunk_logged = Arc::new(AtomicBool::new(false));
    let first_chunk_logged_for_stream = Arc::clone(&first_chunk_logged);
    let asset_for_stream = asset.to_string();
    let relay_token_id_for_stream = relay_token_id.to_string();
    let upstream_host_for_stream = upstream_host.clone();
    let body = Body::from_stream(response.bytes_stream().inspect(move |result| {
        if first_chunk_logged_for_stream.swap(true, Ordering::Relaxed) {
            return;
        }

        match result {
            Ok(chunk) => {
                debug!(
                    component = "euripus-relay",
                    asset = %asset_for_stream,
                    relay_token_id = %relay_token_id_for_stream,
                    upstream_host = %upstream_host_for_stream,
                    first_chunk_ms = started_at.elapsed().as_millis(),
                    first_chunk_bytes = chunk.len(),
                    "received first upstream stream chunk"
                );
            }
            Err(error) => {
                warn!(
                    component = "iptv-provider",
                    asset = %asset_for_stream,
                    relay_token_id = %relay_token_id_for_stream,
                    upstream_host = %upstream_host_for_stream,
                    first_chunk_ms = started_at.elapsed().as_millis(),
                    error = ?error,
                    "upstream stream failed before the first chunk reached the relay response body"
                );
            }
        }
    }));

    let builder = relay_response_headers(
        Response::builder().status(status),
        &upstream_headers,
        &[
            "content-type",
            "content-length",
            "content-range",
            "accept-ranges",
            "etag",
            "last-modified",
            "cache-control",
        ],
    );

    builder
        .body(body)
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

pub(super) fn relay_upstream_request(
    client: &reqwest::Client,
    upstream_url: Url,
    incoming_headers: &HeaderMap,
    forwarded_headers: &[&str],
) -> reqwest::RequestBuilder {
    let mut request = client.get(upstream_url);
    let mut forwarded_user_agent = false;

    for header_name in forwarded_headers {
        if let Some(value) = incoming_headers
            .get(*header_name)
            .and_then(|value| value.to_str().ok())
        {
            request = request.header(*header_name, value);
            if *header_name == "user-agent" {
                forwarded_user_agent = true;
            }
        }
    }

    if !forwarded_user_agent {
        request = request.header(header::USER_AGENT, "EuripusRelay/1.0");
    }

    request
}

fn relay_response_headers(
    mut builder: axum::http::response::Builder,
    upstream_headers: &reqwest::header::HeaderMap,
    passed_headers: &[&str],
) -> axum::http::response::Builder {
    for header_name in passed_headers {
        if let Some(value) = upstream_headers
            .get(*header_name)
            .and_then(|value| value.to_str().ok())
        {
            builder = builder.header(*header_name, value);
        }
    }

    builder
}

const PROVIDER_PLACEHOLDER_ERROR: &str = "provider-placeholder";
const PROVIDER_PLACEHOLDER_STATUS: u16 = 460;

fn is_provider_placeholder_manifest(upstream_base_url: &Url, manifest: &str) -> bool {
    let mut has_extm3u_header = false;
    let mut saw_content = false;
    let mut has_end_list = false;
    let mut has_master_semantics = false;
    let mut extinf_count = 0;
    let mut pending_extinf = false;
    let mut malformed_segment_metadata = false;
    let mut paired_segment_count = 0;
    let mut media_uris = Vec::new();

    for raw_line in manifest.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let is_first_content = !saw_content;
        saw_content = true;
        if line.starts_with('#') {
            let upper = line.to_ascii_uppercase();
            if is_first_content {
                has_extm3u_header = upper == "#EXTM3U";
            }
            if upper == "#EXT-X-ENDLIST" {
                has_end_list = true;
                malformed_segment_metadata |= pending_extinf;
            }
            has_master_semantics |= upper.starts_with("#EXT-X-STREAM-INF")
                || upper.starts_with("#EXT-X-I-FRAME-STREAM-INF")
                || upper.starts_with("#EXT-X-MEDIA:");
            if upper.starts_with("#EXTINF:") {
                malformed_segment_metadata |= pending_extinf;
                extinf_count += 1;
                pending_extinf = true;
            }
            continue;
        }
        if pending_extinf {
            paired_segment_count += 1;
            pending_extinf = false;
        }
        media_uris.push(line);
    }

    if !has_extm3u_header
        || !has_end_list
        || has_master_semantics
        || malformed_segment_metadata
        || pending_extinf
        || media_uris.len() != 1
        || extinf_count != 1
        || paired_segment_count != 1
    {
        return false;
    }

    let resolved = Url::parse(media_uris[0]).or_else(|_| upstream_base_url.join(media_uris[0]));
    resolved
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|segments| segments.last())
                .map(ToString::to_string)
        })
        .is_some_and(|name| name.eq_ignore_ascii_case("black.ts"))
}

fn provider_placeholder_response() -> Result<Response, AppError> {
    Response::builder()
        .status(
            StatusCode::from_u16(PROVIDER_PLACEHOLDER_STATUS)
                .expect("provider placeholder status is valid"),
        )
        .header(header::CACHE_CONTROL, "no-store")
        .header("x-euripus-playback-error", PROVIDER_PLACEHOLDER_ERROR)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(
            "This channel is currently unavailable from the provider.",
        ))
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

pub(super) fn rewrite_hls_manifest(
    state: &AppState,
    user_id: Uuid,
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
    public_base_url: &Url,
    upstream_base_url: &Url,
    manifest: &str,
) -> Result<String, AppError> {
    let mut rewritten_lines = Vec::new();

    for line in manifest.lines() {
        if line.starts_with('#') {
            rewritten_lines.push(rewrite_hls_tag_uris(
                state,
                user_id,
                profile_id,
                expires_at,
                public_base_url,
                upstream_base_url,
                line,
            )?);
            continue;
        }

        if line.trim().is_empty() {
            rewritten_lines.push(line.to_string());
            continue;
        }

        rewritten_lines.push(rewrite_hls_media_uri(
            state,
            user_id,
            profile_id,
            expires_at,
            public_base_url,
            upstream_base_url,
            line,
        )?);
    }

    Ok(rewritten_lines.join("\n"))
}

fn rewrite_hls_tag_uris(
    state: &AppState,
    user_id: Uuid,
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
    public_base_url: &Url,
    upstream_base_url: &Url,
    line: &str,
) -> Result<String, AppError> {
    let mut output = String::new();
    let mut remaining = line;

    while let Some(start) = remaining.find("URI=\"") {
        let attribute_start = start + 5;
        output.push_str(&remaining[..attribute_start]);
        let rest = &remaining[attribute_start..];
        let Some(end) = rest.find('"') else {
            output.push_str(rest);
            return Ok(output);
        };
        let uri = &rest[..end];
        let rewritten = relayable_uri_to_public_url(
            state,
            user_id,
            profile_id,
            expires_at,
            public_base_url,
            upstream_base_url,
            uri,
        )?
        .unwrap_or_else(|| uri.to_string());
        output.push_str(&rewritten);
        output.push('"');
        remaining = &rest[end + 1..];
    }

    output.push_str(remaining);
    Ok(output)
}

fn rewrite_hls_media_uri(
    state: &AppState,
    user_id: Uuid,
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
    public_base_url: &Url,
    upstream_base_url: &Url,
    uri: &str,
) -> Result<String, AppError> {
    Ok(relayable_uri_to_public_url(
        state,
        user_id,
        profile_id,
        expires_at,
        public_base_url,
        upstream_base_url,
        uri.trim(),
    )?
    .unwrap_or_else(|| uri.to_string()))
}

fn relayable_uri_to_public_url(
    state: &AppState,
    user_id: Uuid,
    profile_id: Uuid,
    expires_at: DateTime<Utc>,
    public_base_url: &Url,
    upstream_base_url: &Url,
    raw_uri: &str,
) -> Result<Option<String>, AppError> {
    let resolved = if let Ok(url) = Url::parse(raw_uri) {
        url
    } else if let Ok(url) = upstream_base_url.join(raw_uri) {
        url
    } else {
        return Ok(None);
    };

    if !matches!(resolved.scheme(), "http" | "https") {
        return Ok(None);
    }

    let kind = relay_asset_kind_for_url(&resolved);
    let token = issue_relay_token(
        state,
        user_id,
        profile_id,
        resolved.as_str(),
        kind,
        Some(expires_at),
    )?;
    Ok(Some(relay_url_for_token(
        public_base_url,
        kind,
        &token.token,
    )?))
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
