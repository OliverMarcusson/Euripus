use std::{
    collections::{HashMap, HashSet, VecDeque},
    fs,
    net::{IpAddr, SocketAddr},
    path::Path as FsPath,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::config::Config;
use crate::epg::{EPG_RETENTION_FUTURE_DAYS, EPG_RETENTION_PAST_HOURS};
use crate::xmltv::{XmltvChannel, XmltvFeed, XmltvProgramme};
use crate::xtreme::{XtreamCategory, XtreamChannel, XtreamCredentials};
use crate::{xmltv, xtreme};
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use anyhow::{Context, Result, anyhow};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderName, HeaderValue, Method, StatusCode, header},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::{delete, get, post, put},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, Utc};
use cookie::time::Duration as CookieDuration;
use dashmap::{DashMap, DashSet};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use meilisearch_sdk::{
    client::Client as MeilisearchClient,
    documents::DocumentDeletionQuery,
    search::{MatchingStrategies, SearchResults},
    settings::{MinWordSizeForTypos, PaginationSetting, TypoToleranceSettings},
    task_info::TaskInfo,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384};
use sqlx::{FromRow, PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use tokio::signal;
use tokio::sync::{RwLock, broadcast};
use tokio::task::{JoinHandle, JoinSet};
use tokio_retry::{Retry, strategy::ExponentialBackoff};
use tokio_stream::{StreamExt as TokioStreamExt, wrappers::BroadcastStream};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info, warn};
use url::Url;
use uuid::Uuid;

mod app;
mod auth;
mod error;
mod guide;
mod playback;
mod provider;
mod receiver;
mod relay;
mod search;
mod state;
mod sync;

use self::playback::relay_tokens::{issue_relay_token, relay_url_for_token};
use self::playback::resolve::{
    normalize_output_format, normalize_playback_mode, output_format_as_str, playback_mode_as_str,
    should_force_relay_for_secure_request,
};
use self::receiver::{ReceiverSessionRecord, load_receiver_device};

pub use app::run;
use error::{ApiResult, AppError};
use state::{AppState, MeiliReadiness, MeiliSetup, SearchIndexCounts};

#[derive(Debug, Serialize, FromRow, Clone)]
#[serde(rename_all = "camelCase")]
struct ChannelResponse {
    id: Uuid,
    #[serde(skip_serializing)]
    profile_id: Uuid,
    name: String,
    logo_url: Option<String>,
    category_name: Option<String>,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_epg: bool,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
    is_favorite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChannelVisibility {
    is_hidden: bool,
    is_placeholder: bool,
}

#[derive(Debug, Serialize, FromRow, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgramResponse {
    id: Uuid,
    channel_id: Option<Uuid>,
    channel_name: Option<String>,
    title: String,
    description: Option<String>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    can_catchup: bool,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct SyncJobResponse {
    id: Uuid,
    status: String,
    job_type: String,
    trigger: String,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    current_phase: Option<String>,
    completed_phases: i32,
    total_phases: i32,
    phase_message: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServerNetworkStatusResponse {
    server_status: String,
    vpn_active: bool,
    vpn_provider: Option<String>,
    public_ip: Option<String>,
    public_ip_checked_at: DateTime<Utc>,
    public_ip_error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchBackendStatusResponse {
    meilisearch: String,
    progress_percent: Option<i32>,
    indexed_documents: Option<i64>,
    total_documents: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelSearchResponse {
    query: String,
    backend: String,
    items: Vec<ChannelResponse>,
    total_count: i64,
    next_offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramSearchResponse {
    query: String,
    backend: String,
    items: Vec<ProgramResponse>,
    total_count: i64,
    next_offset: Option<i64>,
}

#[derive(Debug, FromRow)]
struct SearchDocumentCountsRow {
    channel_documents: i64,
    program_documents: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchQuery {
    q: String,
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AccessClaims {
    sub: String,
    sid: String,
    username: String,
    exp: usize,
}

#[derive(Clone)]
struct AuthContext {
    user_id: Uuid,
    session_id: Uuid,
}

#[derive(Clone)]
struct ReceiverAuthContext {
    receiver_device_id: Uuid,
    receiver_session_id: Uuid,
}

#[derive(Debug, FromRow, Clone)]
struct ProviderProfileRecord {
    id: Uuid,
    user_id: Uuid,
    provider_type: String,
    base_url: String,
    username: String,
    password_encrypted: String,
    output_format: String,
    playback_mode: String,
    status: String,
    last_validated_at: Option<DateTime<Utc>>,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
struct EpgSourceRecord {
    id: Uuid,
    url: String,
    priority: i32,
    source_kind: String,
}

#[derive(Debug, Clone, FromRow)]
struct PersistedChannelRecord {
    id: Uuid,
    name: String,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_catchup: bool,
}

#[derive(Debug, Clone, FromRow)]
struct PersistedChannelSyncRow {
    id: Uuid,
    remote_stream_id: i32,
    category_remote_id: Option<String>,
    name: String,
    logo_url: Option<String>,
    epg_channel_id: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct PersistedCategoryRecord {
    remote_category_id: String,
    name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MeiliChannelDoc {
    id: String,
    user_id: String,
    profile_id: String,
    entity_id: String,
    channel_name: String,
    subtitle: Option<String>,
    category_name_raw: Option<String>,
    country_code: Option<String>,
    region_code: Option<String>,
    provider_key: Option<String>,
    provider_labels: Vec<String>,
    broad_categories: Vec<String>,
    event_titles: Vec<String>,
    event_keywords: Vec<String>,
    is_event_channel: bool,
    is_placeholder_channel: bool,
    is_hidden: bool,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    epg_channel_id: Option<String>,
    search_text: String,
    sort_rank: i32,
    updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct MeiliProgramDoc {
    id: String,
    user_id: String,
    profile_id: String,
    entity_id: String,
    country_code: Option<String>,
    region_code: Option<String>,
    provider_key: Option<String>,
    provider_labels: Vec<String>,
    broad_categories: Vec<String>,
    channel_name: Option<String>,
    title: String,
    description: Option<String>,
    search_text: String,
    starts_at: i64,
    ends_at: i64,
    can_catchup: bool,
    channel_id: Option<String>,
    is_hidden: bool,
    sort_priority: i32,
}

#[derive(Debug, Clone)]
struct ChannelProgramMetadata {
    country_code: Option<String>,
    region_code: Option<String>,
    provider_key: Option<String>,
    provider_labels: Vec<String>,
    broad_categories: Vec<String>,
    is_hidden: bool,
}

#[derive(Debug)]
struct PendingMeiliBatch {
    task: TaskInfo,
    phase: &'static str,
    batch_number: usize,
    indexed_documents: usize,
}

#[derive(Debug, FromRow)]
struct MeiliChannelRow {
    id: Uuid,
    profile_id: Uuid,
    name: String,
    category_name: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    epg_channel_id: Option<String>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct MeiliProgramRow {
    id: Uuid,
    profile_id: Uuid,
    channel_id: Option<Uuid>,
    channel_name: Option<String>,
    category_name: Option<String>,
    title: String,
    description: Option<String>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    can_catchup: bool,
}

#[derive(Debug, FromRow)]
struct ChannelEventTitlesRow {
    channel_id: Uuid,
    titles: Vec<String>,
}

#[derive(Debug, FromRow)]
struct ChannelVisibilityRow {
    id: Uuid,
    name: String,
    category_name: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RelayAssetKind {
    Hls,
    Raw,
    Asset,
}

const SYNC_BATCH_SIZE: usize = 10_000;
const EPG_FETCH_CONCURRENCY: usize = 4;
const CHANNEL_SYNC_TOTAL_PHASES: i32 = 4;
const FULL_SYNC_TOTAL_PHASES: i32 = 7;
const EPG_SYNC_TOTAL_PHASES: i32 = 4;
const SEARCH_DEFAULT_LIMIT: i64 = 30;
const SEARCH_MAX_LIMIT: i64 = 100;
const REFRESH_COOKIE_NAME: &str = "euripus.refresh";
const CSRF_COOKIE_NAME: &str = "euripus.csrf";
const CSRF_HEADER_NAME: &str = "x-csrf-token";
const RELAY_PLAYLIST_MAX_BYTES: usize = 1024 * 1024;
const PUBLIC_IP_LOOKUP_URL: &str = "https://api.ipify.org";
const PUBLIC_IP_LOOKUP_TIMEOUT_SECONDS: u64 = 5;
const RELAY_UPSTREAM_CONNECT_TIMEOUT_SECONDS: u64 = 10;
const RELAY_UPSTREAM_READ_TIMEOUT_SECONDS: u64 = 25;
const INTERRUPTED_SYNC_MESSAGE: &str =
    "Sync was interrupted when the server restarted. Start a new sync.";
const DATABASE_STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
const DATABASE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DATABASE_RETRY_DELAY_INITIAL: Duration = Duration::from_secs(2);
const DATABASE_RETRY_DELAY_MAX: Duration = Duration::from_secs(10);
const MEILI_STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const SESSION_CACHE_TTL: Duration = Duration::from_secs(30);
const RELAY_PROFILE_CACHE_TTL: Duration = Duration::from_secs(10);
const PERIODIC_CHANNEL_SYNC_INTERVAL: Duration = Duration::from_secs(60 * 3);
const MEILI_INDEX_BATCH_SIZE: i64 = 10_000;
const MEILI_MAX_TOTAL_HITS: usize = 20_000;
const MEILI_MAX_IN_FLIGHT_TASKS: usize = 2;
const MEILI_TASK_POLL_INTERVAL: Duration = Duration::from_millis(250);
const MEILI_TASK_TIMEOUT: Duration = Duration::from_secs(300);
const MEILI_SCHEMA_VERSION_KEY: &str = "__euripus_schema_version__";
const MEILI_SCHEMA_VERSION: &str = "v3";
const RECEIVER_TTL: Duration = Duration::from_secs(45);
const RECEIVER_SESSION_TTL_HOURS: i64 = 12;
const RECEIVER_PAIRING_CODE_MINUTES: i64 = 5;

async fn lookup_public_ip(client: &reqwest::Client) -> Result<String, anyhow::Error> {
    let response = client
        .get(PUBLIC_IP_LOOKUP_URL)
        .timeout(Duration::from_secs(PUBLIC_IP_LOOKUP_TIMEOUT_SECONDS))
        .send()
        .await
        .context("failed to request public IP")?;
    let public_ip = response
        .text()
        .await
        .context("failed to read public IP response body")?
        .trim()
        .to_string();
    IpAddr::from_str(&public_ip)
        .with_context(|| format!("public IP lookup returned an invalid IP address: {public_ip}"))?;

    Ok(public_ip.to_string())
}

async fn get_server_network_status(
    State(state): State<AppState>,
) -> ApiResult<ServerNetworkStatusResponse> {
    let public_ip_checked_at = Utc::now();
    let (public_ip, public_ip_error) = match lookup_public_ip(&state.provider_http_client).await {
        Ok(public_ip) => (Some(public_ip), None),
        Err(error) => {
            warn!("public IP lookup failed: {error:?}");
            (
                None,
                Some("Public IP lookup is temporarily unavailable.".to_string()),
            )
        }
    };

    Ok(Json(ServerNetworkStatusResponse {
        server_status: "online".to_string(),
        vpn_active: state.config.vpn_enabled,
        vpn_provider: state.config.vpn_provider_name.clone(),
        public_ip,
        public_ip_checked_at,
        public_ip_error,
    }))
}

fn json_response_with_revalidation<T: Serialize>(
    headers: &HeaderMap,
    payload: &T,
) -> Result<Response, AppError> {
    let body = serde_json::to_vec(payload).map_err(|error| AppError::Internal(anyhow!(error)))?;
    let etag = format!("\"{:x}\"", Sha256::digest(&body));
    let builder = Response::builder()
        .header(header::CACHE_CONTROL, "private, no-cache")
        .header(header::ETAG, &etag);

    if if_none_match_matches(headers, &etag) {
        return builder
            .status(StatusCode::NOT_MODIFIED)
            .body(Body::empty())
            .map_err(|error| AppError::Internal(anyhow!(error)));
    }

    builder
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

fn if_none_match_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(|value| {
            value
                .split(',')
                .any(|candidate| matches_etag_candidate(candidate, etag))
        })
        .unwrap_or(false)
}

fn matches_etag_candidate(candidate: &str, etag: &str) -> bool {
    let candidate = candidate.trim();
    candidate == "*" || candidate == etag
}

async fn fetch_channels(pool: &PgPool, user_id: Uuid) -> Result<Vec<ChannelResponse>> {
    let channels = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          EXISTS(
            SELECT 1
            FROM programs p
            WHERE p.user_id = c.user_id
              AND p.channel_id = c.id
              AND p.end_at > NOW() - ($2 * INTERVAL '1 hour')
              AND p.start_at < NOW() + ($3 * INTERVAL '1 day')
          ) AS has_epg,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1
        ORDER BY c.name ASC
        "#,
    )
    .bind(user_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(pool)
    .await?;

    Ok(channels)
}

fn normalize_visibility_text(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    let mut previous_was_space = true;

    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            normalized.push(ch);
            previous_was_space = false;
        } else if !previous_was_space {
            normalized.push(' ');
            previous_was_space = true;
        }
    }

    normalized.trim().to_string()
}

fn contains_visibility_phrase(text: &str, phrase: &str) -> bool {
    if text == phrase {
        return true;
    }

    text.starts_with(&format!("{phrase} "))
        || text.ends_with(&format!(" {phrase}"))
        || text.contains(&format!(" {phrase} "))
}

fn text_has_placeholder_marker(value: &str) -> bool {
    if value.contains('#') {
        return true;
    }

    let normalized = normalize_visibility_text(value);
    [
        "ended",
        "no event streaming",
        "event only",
        "streaming now",
        "no live event",
    ]
    .iter()
    .any(|phrase| contains_visibility_phrase(&normalized, phrase))
}

fn text_has_ppv_branding(value: &str) -> bool {
    let normalized = normalize_visibility_text(value);
    contains_visibility_phrase(&normalized, "ppv")
        || contains_visibility_phrase(&normalized, "pay per view")
}

fn token_has_numeric_suffix(token: &str) -> bool {
    let digit_start = token.find(|ch: char| ch.is_ascii_digit());
    let Some(digit_start) = digit_start else {
        return false;
    };

    let (prefix, suffix) = token.split_at(digit_start);
    !suffix.is_empty()
        && suffix.chars().all(|ch| ch.is_ascii_digit())
        && prefix.chars().all(|ch| ch.is_ascii_alphabetic())
        && prefix.len() <= 4
}

fn looks_like_generic_numbered_ppv(channel_name: &str) -> bool {
    let normalized = normalize_visibility_text(channel_name);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    let Some(last_token) = tokens.last().copied() else {
        return false;
    };

    let has_numbered_suffix =
        last_token.chars().all(|ch| ch.is_ascii_digit()) || token_has_numeric_suffix(last_token);
    has_numbered_suffix && tokens.len() <= 4
}

fn has_meaningful_event_title(event_titles: &[String]) -> bool {
    event_titles.iter().any(|title| {
        let normalized = normalize_visibility_text(title);
        !normalized.is_empty()
            && !text_has_placeholder_marker(title)
            && normalized != "ppv"
            && normalized != "event"
            && normalized != "live event"
    })
}

fn channel_name_has_event_prefix(channel_name: &str) -> bool {
    let normalized = normalize_visibility_text(channel_name);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    if tokens.len() < 3 {
        return false;
    }

    matches!(tokens.first().copied(), Some("live" | "next"))
}

fn month_token_to_number(token: &str) -> Option<u32> {
    match token {
        "jan" | "january" => Some(1),
        "feb" | "february" => Some(2),
        "mar" | "march" => Some(3),
        "apr" | "april" => Some(4),
        "may" => Some(5),
        "jun" | "june" => Some(6),
        "jul" | "july" => Some(7),
        "aug" | "august" => Some(8),
        "sep" | "sept" | "september" => Some(9),
        "oct" | "october" => Some(10),
        "nov" | "november" => Some(11),
        "dec" | "december" => Some(12),
        _ => None,
    }
}

fn parse_day_token(token: &str) -> Option<u32> {
    let day = token.parse::<u32>().ok()?;
    (1..=31).contains(&day).then_some(day)
}

fn extract_channel_event_date(channel_name: &str, year: i32) -> Option<NaiveDate> {
    let normalized = normalize_visibility_text(channel_name);
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();

    for (index, token) in tokens.iter().enumerate() {
        let Some(month) = month_token_to_number(token) else {
            continue;
        };

        if let Some(day) = tokens
            .get(index + 1)
            .and_then(|value| parse_day_token(value))
        {
            if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                return Some(date);
            }
        }

        if let Some(day) = index
            .checked_sub(1)
            .and_then(|day_index| tokens.get(day_index))
            .and_then(|value| parse_day_token(value))
        {
            if let Some(date) = NaiveDate::from_ymd_opt(year, month, day) {
                return Some(date);
            }
        }
    }

    None
}

fn classify_channel_visibility_at(
    channel_name: &str,
    category_name: Option<&str>,
    event_titles: &[String],
    today: NaiveDate,
) -> ChannelVisibility {
    let is_placeholder = text_has_placeholder_marker(channel_name)
        || category_name
            .map(text_has_placeholder_marker)
            .unwrap_or(false);
    if is_placeholder {
        return ChannelVisibility {
            is_hidden: true,
            is_placeholder: true,
        };
    }

    let is_ppv = text_has_ppv_branding(channel_name)
        || category_name.map(text_has_ppv_branding).unwrap_or(false);
    let has_past_event_date = extract_channel_event_date(channel_name, today.year())
        .map(|event_date| event_date < today)
        .unwrap_or(false);
    let is_hidden = is_ppv
        && ((looks_like_generic_numbered_ppv(channel_name)
            && !has_meaningful_event_title(event_titles)
            && !channel_name_has_event_prefix(channel_name))
            || has_past_event_date);

    ChannelVisibility {
        is_hidden,
        is_placeholder: false,
    }
}

fn classify_channel_visibility(
    channel_name: &str,
    category_name: Option<&str>,
    event_titles: &[String],
) -> ChannelVisibility {
    classify_channel_visibility_at(
        channel_name,
        category_name,
        event_titles,
        Utc::now().date_naive(),
    )
}

async fn load_channel_visibility_map(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Option<Uuid>,
) -> Result<HashMap<Uuid, ChannelVisibility>> {
    let rows = sqlx::query_as::<_, ChannelVisibilityRow>(
        r#"
        SELECT
          c.id,
          c.name,
          cc.name AS category_name
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1
          AND ($2::uuid IS NULL OR c.profile_id = $2)
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_all(pool)
    .await?;

    let titles = sqlx::query_as::<_, ChannelEventTitlesRow>(
        r#"
        WITH ranked AS (
          SELECT
            p.channel_id,
            p.title,
            ROW_NUMBER() OVER (
              PARTITION BY p.channel_id
              ORDER BY
                CASE
                  WHEN p.start_at <= NOW() AND p.end_at >= NOW() THEN 0
                  WHEN p.start_at > NOW() THEN 1
                  ELSE 2
                END,
                p.start_at ASC,
                p.title ASC
            ) AS rank
          FROM programs p
          WHERE p.user_id = $1
            AND p.channel_id IS NOT NULL
            AND ($2::uuid IS NULL OR p.profile_id = $2)
            AND p.end_at > NOW() - ($3 * INTERVAL '1 hour')
            AND p.start_at < NOW() + ($4 * INTERVAL '1 day')
        )
        SELECT channel_id, array_agg(title ORDER BY rank) AS titles
        FROM ranked
        WHERE rank <= 3
        GROUP BY channel_id
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .bind(EPG_RETENTION_PAST_HOURS)
    .bind(EPG_RETENTION_FUTURE_DAYS)
    .fetch_all(pool)
    .await?;
    let titles_by_channel = titles
        .into_iter()
        .map(|row| (row.channel_id, row.titles))
        .collect::<HashMap<_, _>>();

    Ok(rows
        .into_iter()
        .map(|row| {
            let event_titles = titles_by_channel.get(&row.id).cloned().unwrap_or_default();
            (
                row.id,
                classify_channel_visibility(&row.name, row.category_name.as_deref(), &event_titles),
            )
        })
        .collect())
}

fn visible_channel_ids_from_map(visibility: &HashMap<Uuid, ChannelVisibility>) -> Vec<Uuid> {
    visibility
        .iter()
        .filter_map(|(id, visibility)| (!visibility.is_hidden).then_some(*id))
        .collect()
}

fn rewrite_channel_logo_urls(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    channels: &mut [ChannelResponse],
) -> Result<(), AppError> {
    let request_base_url = request_base_url(&state.config, headers)?;
    for channel in channels {
        channel.logo_url = rewrite_channel_logo_url(
            state,
            &request_base_url,
            user_id,
            channel.profile_id,
            channel.logo_url.take(),
        )?;
    }

    Ok(())
}

fn rewrite_channel_logo_url(
    state: &AppState,
    request_base_url: &Url,
    user_id: Uuid,
    profile_id: Uuid,
    logo_url: Option<String>,
) -> Result<Option<String>, AppError> {
    let Some(logo_url) = logo_url else {
        return Ok(None);
    };

    if !should_force_relay_for_secure_request(request_base_url, &logo_url) {
        return Ok(Some(logo_url));
    }

    let relay_token = issue_relay_token(
        state,
        user_id,
        profile_id,
        &logo_url,
        RelayAssetKind::Asset,
        None,
    )?;
    let relay_url =
        relay_url_for_token(request_base_url, RelayAssetKind::Asset, &relay_token.token)?;

    Ok(Some(relay_url))
}

async fn touch_recent(pool: &PgPool, user_id: Uuid, channel_id: Uuid) -> Result<()> {
    sqlx::query(
        r#"
        INSERT INTO recents (user_id, channel_id, last_played_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (user_id, channel_id)
        DO UPDATE SET last_played_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(channel_id)
    .execute(pool)
    .await?;
    Ok(())
}

fn request_base_url(config: &Config, headers: &HeaderMap) -> Result<Url, AppError> {
    if let Some(origin) = &config.public_origin {
        return Ok(origin.clone());
    }

    let forwarded_host = header_value(headers, "x-forwarded-host");
    let host_header = header_value(headers, "host");
    let host = forwarded_host.as_deref().or(host_header.as_deref());
    let scheme = header_value(headers, "x-forwarded-proto").unwrap_or_else(|| "http".to_string());

    if let Some(host) = host {
        return Url::parse(&format!("{scheme}://{host}"))
            .map_err(|error| AppError::Internal(anyhow!(error)));
    }

    Url::parse(&format!("http://{}", config.bind_address))
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

async fn require_receiver_auth_with_optional_query_token(
    state: &AppState,
    headers: &HeaderMap,
    session_token: Option<String>,
) -> Result<ReceiverAuthContext, AppError> {
    if let Some(token) = session_token.filter(|token| !token.is_empty()) {
        return receiver_auth_from_session_token(state, &token).await;
    }

    require_receiver_auth(state, headers).await
}

async fn require_receiver_auth(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<ReceiverAuthContext, AppError> {
    let header_value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;
    receiver_auth_from_session_token(state, token).await
}

async fn receiver_auth_from_session_token(
    state: &AppState,
    token: &str,
) -> Result<ReceiverAuthContext, AppError> {
    let session = sqlx::query_as::<_, ReceiverSessionRecord>(
        r#"
        SELECT id, receiver_device_id
        FROM receiver_sessions
        WHERE session_token_hash = $1 AND closed_at IS NULL AND expires_at > NOW()
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(hash_receiver_token(token))
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    let device = load_receiver_device(&state.pool, session.receiver_device_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if device.revoked_at.is_some() {
        return Err(AppError::Unauthorized);
    }

    Ok(ReceiverAuthContext {
        receiver_device_id: session.receiver_device_id,
        receiver_session_id: session.id,
    })
}

async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<AuthContext, AppError> {
    let header_value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;
    auth_context_from_access_token(state, token).await
}

async fn auth_context_from_access_token(
    state: &AppState,
    token: &str,
) -> Result<AuthContext, AppError> {
    let claims = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
    let session_id = Uuid::parse_str(&claims.sid).map_err(|_| AppError::Unauthorized)?;
    let cache_key = (session_id, user_id);
    let now = Instant::now();
    let cached_expiry = state.session_cache.get(&cache_key).map(|expiry| *expiry);
    if let Some(expiry) = cached_expiry {
        if expiry > now {
            return Ok(AuthContext {
                user_id,
                session_id,
            });
        }
        state.session_cache.remove(&cache_key);
    }
    let valid_session = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM sessions
        WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL AND expires_at > NOW()
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    if valid_session == 0 {
        return Err(AppError::Unauthorized);
    }

    state
        .session_cache
        .insert(cache_key, now + SESSION_CACHE_TTL);

    Ok(AuthContext {
        user_id,
        session_id,
    })
}

fn create_access_token(
    state: &AppState,
    user_id: Uuid,
    username: &str,
    session_id: Uuid,
) -> Result<(String, DateTime<Utc>)> {
    let expires_at = Utc::now() + ChronoDuration::minutes(state.config.access_token_minutes);
    let claims = AccessClaims {
        sub: user_id.to_string(),
        sid: session_id.to_string(),
        username: username.to_string(),
        exp: expires_at.timestamp() as usize,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )?;

    Ok((token, expires_at))
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hashed = Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow!(error.to_string()))?
        .to_string();
    Ok(hashed)
}

fn verify_password(password_hash: &str, password: &str) -> Result<()> {
    let parsed_hash =
        PasswordHash::new(password_hash).map_err(|error| anyhow!(error.to_string()))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| anyhow!("invalid credentials"))?;
    Ok(())
}

fn generate_refresh_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

fn hash_refresh_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex_encode(&hasher.finalize())
}

fn hash_receiver_token(token: &str) -> String {
    hash_refresh_token(token)
}

fn generate_pairing_code() -> String {
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut bytes = [0u8; 4];
    rand::rng().fill_bytes(&mut bytes);
    bytes
        .iter()
        .map(|byte| CHARSET[(*byte as usize) % CHARSET.len()] as char)
        .collect()
}

fn encrypt_secret(key: &[u8; 32], value: &str) -> Result<String> {
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let mut nonce_bytes = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, value.as_bytes())
        .map_err(|error| anyhow!(error.to_string()))?;

    let mut payload = nonce_bytes.to_vec();
    payload.extend(ciphertext);
    Ok(STANDARD.encode(payload))
}

fn decrypt_secret(key: &[u8; 32], value: &str) -> Result<String> {
    let payload = STANDARD.decode(value)?;
    if payload.len() < 13 {
        return Err(anyhow!("encrypted payload too short"));
    }

    let (nonce_bytes, ciphertext) = payload.split_at(12);
    let cipher = Aes256Gcm::new_from_slice(key)?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|error| anyhow!(error.to_string()))?;
    Ok(String::from_utf8(plaintext)?)
}

fn hex_encode(bytes: &[u8]) -> String {
    const LUT: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(LUT[(byte >> 4) as usize] as char);
        output.push(LUT[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
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
        let visibility =
            classify_channel_visibility(":Viaplay SE 13", Some("SE| VIAPLAY PPV"), &[]);

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

        let response = json_response_with_revalidation(&HeaderMap::new(), &payload)
            .expect("cached json response");

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
}
