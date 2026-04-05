mod config;
mod xmltv;
mod xtreme;

use std::{
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

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
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Duration as ChronoDuration, Local, NaiveDate, Timelike, Utc};
use config::Config;
use cookie::time::Duration as CookieDuration;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256, Sha384};
use sqlx::{FromRow, PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use tokio::signal;
use tokio::task::{JoinHandle, JoinSet};
use tower_http::{
    cors::{AllowOrigin, CorsLayer},
    trace::TraceLayer,
};
use tracing::{error, info, warn};
use url::Url;
use uuid::Uuid;
use xmltv::{XmltvChannel, XmltvFeed, XmltvProgramme};
use xtreme::{XtreamCategory, XtreamChannel, XtreamCredentials};

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    config: Arc<Config>,
    http_client: reqwest::Client,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload {
    error: String,
    message: String,
    status: u16,
}

#[derive(Debug)]
enum AppError {
    Unauthorized,
    NotFound(String),
    BadRequest(String),
    Internal(anyhow::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::Internal(anyhow!(value))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            AppError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                "unauthorized".to_string(),
                "Authentication is required".to_string(),
            ),
            AppError::NotFound(message) => {
                (StatusCode::NOT_FOUND, "not_found".to_string(), message)
            }
            AppError::BadRequest(message) => {
                (StatusCode::BAD_REQUEST, "bad_request".to_string(), message)
            }
            AppError::Internal(error) => {
                error!("internal server error: {error:?}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_server_error".to_string(),
                    "Something went wrong".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorPayload {
                error,
                message,
                status: status.as_u16(),
            }),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<Json<T>, AppError>;

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct UserResponse {
    id: Uuid,
    username: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthSessionResponse {
    user: UserResponse,
    access_token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopAuthSessionResponse {
    #[serde(flatten)]
    session: AuthSessionResponse,
    refresh_token: String,
}

#[derive(Debug)]
struct IssuedSession {
    session: AuthSessionResponse,
    refresh_token: String,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct SessionResponse {
    id: Uuid,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_used_at: Option<DateTime<Utc>>,
    user_agent: Option<String>,
    current: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderProfileResponse {
    id: Uuid,
    provider_type: String,
    base_url: String,
    username: String,
    output_format: String,
    playback_mode: String,
    status: String,
    last_validated_at: Option<DateTime<Utc>>,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    epg_sources: Vec<EpgSourceResponse>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
#[serde(rename_all = "camelCase")]
struct EpgSourceResponse {
    id: Uuid,
    url: String,
    priority: i32,
    enabled: bool,
    source_kind: String,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_error: Option<String>,
    last_program_count: Option<i32>,
    last_matched_count: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

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
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
    is_favorite: bool,
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
struct GuideResponse {
    categories: Vec<GuideCategorySummaryResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuidePreferencesResponse {
    included_category_ids: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GuideCategorySummaryResponse {
    id: String,
    name: String,
    channel_count: i64,
    live_now_count: i64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GuideChannelEntryResponse {
    channel: ChannelResponse,
    program: Option<ProgramResponse>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GuideCategoryResponse {
    category: GuideCategorySummaryResponse,
    entries: Vec<GuideChannelEntryResponse>,
    total_count: i64,
    next_offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChannelSearchResponse {
    query: String,
    items: Vec<ChannelResponse>,
    total_count: i64,
    next_offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgramSearchResponse {
    query: String,
    items: Vec<ProgramResponse>,
    total_count: i64,
    next_offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RecentChannelResponse {
    channel: ChannelResponse,
    last_played_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlaybackSourceResponse {
    kind: String,
    url: String,
    headers: HashMap<String, String>,
    live: bool,
    catchup: bool,
    expires_at: Option<DateTime<Utc>>,
    unsupported_reason: Option<String>,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsPayload {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RefreshPayload {
    refresh_token: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveGuidePreferencesPayload {
    included_category_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GuideCategoryQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveProviderPayload {
    base_url: String,
    username: String,
    password: String,
    output_format: String,
    playback_mode: String,
    #[serde(default)]
    epg_sources: Vec<SaveEpgSourcePayload>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SaveEpgSourcePayload {
    id: Option<Uuid>,
    url: String,
    enabled: bool,
    priority: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateProviderResponse {
    valid: bool,
    status: String,
    message: String,
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

#[derive(Debug, FromRow)]
struct UserRecord {
    id: Uuid,
    username: String,
    password_hash: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct SessionRecord {
    id: Uuid,
    user_id: Uuid,
    refresh_token_hash: String,
    user_agent: Option<String>,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    last_used_at: Option<DateTime<Utc>>,
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
    last_scheduled_sync_on: Option<NaiveDate>,
    last_sync_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
struct EpgSourceRecord {
    id: Uuid,
    profile_id: Uuid,
    url: String,
    priority: i32,
    enabled: bool,
    source_kind: String,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_error: Option<String>,
    last_program_count: Option<i32>,
    last_matched_count: Option<i32>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct PersistedChannelRecord {
    id: Uuid,
    name: String,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_catchup: bool,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
struct ChannelResolution {
    channel_id: Uuid,
    channel_name: String,
    has_catchup: bool,
}

#[derive(Debug, Clone, Default)]
struct ChannelLookupIndex {
    epg_channel_ids: HashMap<String, ChannelResolution>,
    remote_stream_ids: HashMap<String, ChannelResolution>,
    normalized_names: HashMap<String, ChannelResolution>,
    simplified_names: HashMap<String, ChannelResolution>,
}

#[derive(Debug, Clone)]
struct FetchedEpgFeed {
    source_id: Option<Uuid>,
    source_kind: String,
    source_label: String,
    priority: i32,
    feed: XmltvFeed,
}

#[derive(Debug, Clone)]
struct EpgSourceSyncStatus {
    source_id: Uuid,
    last_sync_error: Option<String>,
    last_program_count: Option<i32>,
    last_matched_count: Option<i32>,
    mark_synced: bool,
}

enum ExternalEpgFetchResult {
    Success(FetchedEpgFeed),
    Failure(EpgSourceSyncStatus),
}

#[derive(Debug, Clone)]
struct ResolvedProgramme {
    channel_id: Uuid,
    channel_name: String,
    title: String,
    description: Option<String>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    can_catchup: bool,
}

#[derive(Debug, FromRow)]
struct ChannelPlaybackRecord {
    id: Uuid,
    profile_id: Uuid,
    name: String,
    remote_stream_id: i32,
    stream_extension: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
    playback_mode: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaybackMode {
    Direct,
    Relay,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RelayAssetKind {
    Hls,
    Raw,
    Asset,
}

#[derive(Debug, Serialize, Deserialize)]
struct RelayClaims {
    sub: String,
    pid: String,
    url: String,
    kind: RelayAssetKind,
    exp: usize,
}

#[derive(Debug)]
struct RelayToken {
    token: String,
    expires_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct RelayTokenQuery {
    token: String,
}

const SYNC_BATCH_SIZE: usize = 10_000;
const EPG_FETCH_CONCURRENCY: usize = 4;
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
const INTERRUPTED_SYNC_MESSAGE: &str =
    "Sync was interrupted when the server restarted. Start a new sync.";
const MIGRATION_0007_CURRENT_SQL: &str =
    include_str!("../migrations/0007_program_search_optimizations.sql");
const DATABASE_STARTUP_TIMEOUT: Duration = Duration::from_secs(180);
const DATABASE_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const DATABASE_RETRY_DELAY_INITIAL: Duration = Duration::from_secs(2);
const DATABASE_RETRY_DELAY_MAX: Duration = Duration::from_secs(10);

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Arc::new(Config::from_env()?);
    let pool = wait_for_postgres(&config.database_url).await?;

    repair_migration_0007_checksum(&pool).await?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;
    let recovery = recover_interrupted_syncs(&pool).await?;
    if recovery.recovered_jobs > 0 || recovery.recovered_profiles > 0 {
        warn!(
            "recovered {} interrupted sync job(s) and {} syncing provider profile(s)",
            recovery.recovered_jobs, recovery.recovered_profiles
        );
    }

    if config.daily_sync_hour_local > 23 {
        return Err(anyhow!(
            "APP_DAILY_SYNC_HOUR_LOCAL must be between 0 and 23"
        ));
    }

    let state = AppState {
        pool,
        config,
        http_client: reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()?,
    };

    let periodic_state = state.clone();
    spawn_periodic_sync_worker(periodic_state);

    let bind_address: SocketAddr = state.config.bind_address;
    let cors = build_cors_layer(&state.config)?;
    let router_state = state.clone();
    if let Some(public_origin) = state.config.public_origin.as_ref() {
        info!("Browser public origin configured as {public_origin}");
    }
    let app = Router::new()
        .route("/health", get(health))
        .merge(legacy_auth_router())
        .merge(shared_api_router())
        .nest("/api", browser_api_router())
        .with_state(router_state)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    info!("Euripus server listening on {bind_address}");
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    state.pool.close().await;
    Ok(())
}

async fn wait_for_postgres(database_url: &str) -> Result<PgPool> {
    let startup_deadline = tokio::time::Instant::now() + DATABASE_STARTUP_TIMEOUT;
    let mut retry_delay = DATABASE_RETRY_DELAY_INITIAL;
    let mut attempt = 1;

    loop {
        let connect_future = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url);
        match tokio::time::timeout(DATABASE_CONNECT_TIMEOUT, connect_future).await {
            Ok(Ok(pool)) => {
                if attempt > 1 {
                    info!("connected to PostgreSQL on startup attempt {attempt}");
                }
                return Ok(pool);
            }
            Ok(Err(error)) => {
                if tokio::time::Instant::now() >= startup_deadline {
                    return Err(error)
                        .context("failed to connect to PostgreSQL before startup timeout");
                }
                warn!(
                    "PostgreSQL is not ready yet on startup attempt {attempt}: {error}. Retrying in {}s",
                    retry_delay.as_secs()
                );
            }
            Err(_) => {
                if tokio::time::Instant::now() >= startup_deadline {
                    return Err(anyhow!(
                        "timed out while connecting to PostgreSQL before startup timeout"
                    ));
                }
                warn!(
                    "PostgreSQL connection attempt {attempt} timed out after {}s. Retrying in {}s",
                    DATABASE_CONNECT_TIMEOUT.as_secs(),
                    retry_delay.as_secs()
                );
            }
        }

        tokio::time::sleep(retry_delay).await;
        retry_delay = std::cmp::min(retry_delay * 2, DATABASE_RETRY_DELAY_MAX);
        attempt += 1;
    }
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn shutdown_signal() {
    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = signal::ctrl_c() => {}
        _ = terminate => {}
    }

    info!("shutdown signal received, draining server and closing PostgreSQL pool");
}

async fn repair_migration_0007_checksum(pool: &PgPool) -> Result<()> {
    let migrations_table_exists =
        sqlx::query_scalar::<_, bool>("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
            .fetch_one(pool)
            .await
            .context("failed to check for sqlx migrations table")?;

    if !migrations_table_exists {
        return Ok(());
    }

    let current_checksum = Sha384::digest(MIGRATION_0007_CURRENT_SQL.as_bytes()).to_vec();

    let updated_rows = sqlx::query(
        r#"
        UPDATE _sqlx_migrations
        SET checksum = $1
        WHERE version = 7 AND success = true AND checksum <> $1
        "#,
    )
    .bind(current_checksum)
    .execute(pool)
    .await
    .context("failed to repair migration 0007 checksum")?
    .rows_affected();

    if updated_rows > 0 {
        warn!(
            "repaired sqlx migration checksum for version 7 to match the current immutable migration bytes"
        );
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct InterruptedSyncRecovery {
    recovered_jobs: u64,
    recovered_profiles: u64,
}

async fn recover_interrupted_syncs(pool: &PgPool) -> Result<InterruptedSyncRecovery> {
    let recovered_jobs = sqlx::query(
        r#"
        UPDATE sync_jobs
        SET
          status = 'failed',
          finished_at = NOW(),
          current_phase = 'failed',
          phase_message = $1,
          error_message = $1
        WHERE status IN ('queued', 'running')
        "#,
    )
    .bind(INTERRUPTED_SYNC_MESSAGE)
    .execute(pool)
    .await?
    .rows_affected();

    let recovered_profiles = sqlx::query(
        r#"
        UPDATE provider_profiles
        SET
          status = 'error',
          last_sync_error = $1,
          updated_at = NOW()
        WHERE status = 'syncing'
        "#,
    )
    .bind(INTERRUPTED_SYNC_MESSAGE)
    .execute(pool)
    .await?
    .rows_affected();

    Ok(InterruptedSyncRecovery {
        recovered_jobs,
        recovered_profiles,
    })
}

fn shared_api_router() -> Router<AppState> {
    Router::new()
        .route("/server/network", get(get_server_network_status))
        .route("/me", get(me))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", delete(revoke_session))
        .route("/provider", get(get_provider))
        .route("/provider/validate", post(validate_provider))
        .route("/provider/xtreme", put(save_provider))
        .route("/provider/sync", post(trigger_sync))
        .route("/provider/sync-status", get(get_sync_status))
        .route("/channels", get(list_channels))
        .route("/channels/{id}", get(get_channel))
        .route("/guide", get(get_guide))
        .route(
            "/guide/preferences",
            get(get_guide_preferences).put(save_guide_preferences),
        )
        .route("/guide/category/{category_id}", get(get_guide_category))
        .route("/guide/channel/{id}", get(get_channel_guide))
        .route("/search/channels", get(search_channels))
        .route("/search/programs", get(search_programs))
        .route("/favorites", get(list_favorites))
        .route(
            "/favorites/{channel_id}",
            post(add_favorite).delete(remove_favorite),
        )
        .route("/recents", get(list_recents))
        .route("/playback/channel/{id}", post(play_channel))
        .route("/playback/program/{id}", post(play_program))
}

fn legacy_auth_router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/logout", post(logout))
}

fn browser_api_router() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(browser_register))
        .route("/auth/login", post(browser_login))
        .route("/auth/refresh", post(browser_refresh_session))
        .route("/auth/logout", post(browser_logout))
        .merge(relay_router())
        .merge(shared_api_router())
}

fn relay_router() -> Router<AppState> {
    Router::new()
        .route("/relay/hls", get(relay_hls_playlist))
        .route("/relay/raw", get(relay_raw_stream))
        .route("/relay/asset", get(relay_asset))
}

fn build_cors_layer(config: &Config) -> Result<CorsLayer> {
    let allowed_origins = config
        .allowed_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin).with_context(|| {
                format!("APP_ALLOWED_ORIGINS contains an invalid origin: {origin}")
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(CorsLayer::new()
        .allow_credentials(true)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            header::AUTHORIZATION,
            header::CONTENT_TYPE,
            HeaderName::from_static(CSRF_HEADER_NAME),
        ])
        .allow_origin(AllowOrigin::list(allowed_origins)))
}

async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CredentialsPayload>,
) -> ApiResult<DesktopAuthSessionResponse> {
    let username = payload.username.trim().to_lowercase();
    if username.len() < 3 {
        return Err(AppError::BadRequest(
            "Username must be at least 3 characters".to_string(),
        ));
    }

    let password_hash = hash_password(&payload.password)?;
    let user = sqlx::query_as::<_, UserRecord>(
        r#"
        INSERT INTO users (username, password_hash)
        VALUES ($1, $2)
        RETURNING id, username, password_hash, created_at
        "#,
    )
    .bind(&username)
    .bind(password_hash)
    .fetch_one(&state.pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            AppError::BadRequest("That username is already taken".to_string())
        }
        other => AppError::Internal(anyhow!(other)),
    })?;

    let session = create_session(&state, &headers, &user).await?;
    Ok(Json(desktop_auth_session_response(session)))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CredentialsPayload>,
) -> ApiResult<DesktopAuthSessionResponse> {
    let username = payload.username.trim().to_lowercase();
    let user = sqlx::query_as::<_, UserRecord>(
        r#"SELECT id, username, password_hash, created_at FROM users WHERE username = $1"#,
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invalid username or password".to_string()))?;

    verify_password(&user.password_hash, &payload.password)?;
    let session = create_session(&state, &headers, &user).await?;
    Ok(Json(desktop_auth_session_response(session)))
}

async fn refresh_session(
    State(state): State<AppState>,
    Json(payload): Json<RefreshPayload>,
) -> ApiResult<DesktopAuthSessionResponse> {
    let session = refresh_session_from_token(&state, &payload.refresh_token).await?;
    Ok(Json(desktop_auth_session_response(session)))
}

async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<RefreshPayload>,
) -> Result<StatusCode, AppError> {
    revoke_session_by_refresh_token(&state, &payload.refresh_token).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn browser_register(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<CredentialsPayload>,
) -> Result<(CookieJar, Json<AuthSessionResponse>), AppError> {
    let username = payload.username.trim().to_lowercase();
    if username.len() < 3 {
        return Err(AppError::BadRequest(
            "Username must be at least 3 characters".to_string(),
        ));
    }

    let password_hash = hash_password(&payload.password)?;
    let user = sqlx::query_as::<_, UserRecord>(
        r#"
        INSERT INTO users (username, password_hash)
        VALUES ($1, $2)
        RETURNING id, username, password_hash, created_at
        "#,
    )
    .bind(&username)
    .bind(password_hash)
    .fetch_one(&state.pool)
    .await
    .map_err(|error| match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            AppError::BadRequest("That username is already taken".to_string())
        }
        other => AppError::Internal(anyhow!(other)),
    })?;

    let session = create_session(&state, &headers, &user).await?;
    Ok(browser_auth_response(&state, jar, session))
}

async fn browser_login(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(payload): Json<CredentialsPayload>,
) -> Result<(CookieJar, Json<AuthSessionResponse>), AppError> {
    let username = payload.username.trim().to_lowercase();
    let user = sqlx::query_as::<_, UserRecord>(
        r#"SELECT id, username, password_hash, created_at FROM users WHERE username = $1"#,
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Invalid username or password".to_string()))?;

    verify_password(&user.password_hash, &payload.password)?;
    let session = create_session(&state, &headers, &user).await?;
    Ok(browser_auth_response(&state, jar, session))
}

async fn browser_refresh_session(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<(CookieJar, Json<AuthSessionResponse>), AppError> {
    validate_browser_csrf(&jar, &headers)?;
    let refresh_token = read_browser_refresh_token(&jar)?;
    let session = refresh_session_from_token(&state, &refresh_token).await?;
    Ok(browser_auth_response(&state, jar, session))
}

async fn browser_logout(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<(CookieJar, StatusCode), AppError> {
    validate_browser_csrf(&jar, &headers)?;
    if let Some(refresh_cookie) = jar.get(REFRESH_COOKIE_NAME) {
        revoke_session_by_refresh_token(&state, refresh_cookie.value()).await?;
    }

    Ok((
        clear_browser_auth_cookies(&state, jar),
        StatusCode::NO_CONTENT,
    ))
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<UserResponse> {
    let auth = require_auth(&state, &headers).await?;
    let user = sqlx::query_as::<_, UserResponse>(
        r#"SELECT id, username, created_at FROM users WHERE id = $1"#,
    )
    .bind(auth.user_id)
    .fetch_one(&state.pool)
    .await?;

    Ok(Json(user))
}

async fn lookup_public_ip(client: &reqwest::Client) -> Result<String, anyhow::Error> {
    let response = client
        .get(PUBLIC_IP_LOOKUP_URL)
        .timeout(Duration::from_secs(PUBLIC_IP_LOOKUP_TIMEOUT_SECONDS))
        .send()
        .await
        .context("failed to request public IP")?
        .error_for_status()
        .context("public IP lookup returned an error status")?;
    let public_ip = response
        .text()
        .await
        .context("failed to read public IP response body")?;
    let public_ip = public_ip.trim();
    IpAddr::from_str(public_ip)
        .with_context(|| format!("public IP lookup returned an invalid IP address: {public_ip}"))?;

    Ok(public_ip.to_string())
}

async fn get_server_network_status(
    State(state): State<AppState>,
) -> ApiResult<ServerNetworkStatusResponse> {
    let public_ip_checked_at = Utc::now();
    let (public_ip, public_ip_error) = match lookup_public_ip(&state.http_client).await {
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

async fn list_sessions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<SessionResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let sessions = sqlx::query_as::<_, SessionResponse>(
        r#"
        SELECT
          id,
          created_at,
          expires_at,
          last_used_at,
          user_agent,
          (id = $2) AS current
        FROM sessions
        WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > NOW()
        ORDER BY created_at DESC
        "#,
    )
    .bind(auth.user_id)
    .bind(auth.session_id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(sessions))
}

async fn revoke_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn load_epg_sources(
    pool: &PgPool,
    profile_id: Uuid,
) -> Result<Vec<EpgSourceResponse>, AppError> {
    let sources = sqlx::query_as::<_, EpgSourceResponse>(
        r#"
        SELECT
          id,
          url,
          priority,
          enabled,
          source_kind,
          last_sync_at,
          last_sync_error,
          last_program_count,
          last_matched_count,
          created_at,
          updated_at
        FROM epg_sources
        WHERE profile_id = $1
        ORDER BY priority ASC, created_at ASC
        "#,
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await?;

    Ok(sources)
}

async fn load_provider_profile_response(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Option<ProviderProfileResponse>, AppError> {
    let provider = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_scheduled_sync_on, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    let Some(provider) = provider else {
        return Ok(None);
    };

    Ok(Some(ProviderProfileResponse {
        id: provider.id,
        provider_type: provider.provider_type,
        base_url: provider.base_url,
        username: provider.username,
        output_format: provider.output_format,
        playback_mode: provider.playback_mode,
        status: provider.status,
        last_validated_at: provider.last_validated_at,
        last_sync_at: provider.last_sync_at,
        last_sync_error: provider.last_sync_error,
        created_at: provider.created_at,
        updated_at: provider.updated_at,
        epg_sources: load_epg_sources(pool, provider.id).await?,
    }))
}

fn normalize_playback_mode(raw: &str) -> Result<PlaybackMode, AppError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "direct" => Ok(PlaybackMode::Direct),
        "relay" => Ok(PlaybackMode::Relay),
        _ => Err(AppError::BadRequest(
            "Playback mode must be either 'direct' or 'relay'.".to_string(),
        )),
    }
}

fn playback_mode_as_str(mode: PlaybackMode) -> &'static str {
    match mode {
        PlaybackMode::Direct => "direct",
        PlaybackMode::Relay => "relay",
    }
}

fn normalize_epg_source_payloads(
    payloads: Vec<SaveEpgSourcePayload>,
) -> Result<Vec<SaveEpgSourcePayload>, AppError> {
    let mut deduped = Vec::new();
    let mut seen_urls = HashSet::new();

    let mut ordered = payloads
        .into_iter()
        .map(|payload| SaveEpgSourcePayload {
            id: payload.id,
            url: payload.url.trim().to_string(),
            enabled: payload.enabled,
            priority: payload.priority,
        })
        .filter(|payload| !payload.url.is_empty())
        .collect::<Vec<_>>();
    ordered.sort_by_key(|payload| payload.priority);

    for (index, payload) in ordered.into_iter().enumerate() {
        url::Url::parse(&payload.url).map_err(|_| {
            AppError::BadRequest(format!("Invalid EPG source URL: {}", payload.url))
        })?;
        if seen_urls.insert(payload.url.clone()) {
            deduped.push(SaveEpgSourcePayload {
                id: payload.id,
                url: payload.url,
                enabled: payload.enabled,
                priority: index as i32,
            });
        }
    }

    Ok(deduped)
}

async fn store_epg_sources(
    pool: &PgPool,
    profile_id: Uuid,
    payloads: &[SaveEpgSourcePayload],
) -> Result<(), AppError> {
    let existing_ids =
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM epg_sources WHERE profile_id = $1")
            .bind(profile_id)
            .fetch_all(pool)
            .await?;
    let existing_ids = existing_ids.into_iter().collect::<HashSet<_>>();

    let mut retained_ids = Vec::new();

    for payload in payloads {
        let source_id = match payload.id.filter(|id| existing_ids.contains(id)) {
            Some(source_id) => source_id,
            None => Uuid::new_v4(),
        };
        retained_ids.push(source_id);

        sqlx::query(
            r#"
            INSERT INTO epg_sources (
              id, profile_id, url, priority, enabled, source_kind, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, 'external', NOW())
            ON CONFLICT (id)
            DO UPDATE SET
              url = EXCLUDED.url,
              priority = EXCLUDED.priority,
              enabled = EXCLUDED.enabled,
              source_kind = EXCLUDED.source_kind,
              updated_at = NOW()
            "#,
        )
        .bind(source_id)
        .bind(profile_id)
        .bind(&payload.url)
        .bind(payload.priority)
        .bind(payload.enabled)
        .execute(pool)
        .await?;
    }

    if retained_ids.is_empty() {
        sqlx::query("DELETE FROM epg_sources WHERE profile_id = $1")
            .bind(profile_id)
            .execute(pool)
            .await?;
    } else {
        sqlx::query("DELETE FROM epg_sources WHERE profile_id = $1 AND id <> ALL($2::uuid[])")
            .bind(profile_id)
            .bind(&retained_ids)
            .execute(pool)
            .await?;
    }

    Ok(())
}

async fn get_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Option<ProviderProfileResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let provider = load_provider_profile_response(&state.pool, auth.user_id).await?;

    Ok(Json(provider))
}

async fn validate_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveProviderPayload>,
) -> ApiResult<ValidateProviderResponse> {
    let auth = require_auth(&state, &headers).await?;
    let existing_profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_scheduled_sync_on, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?;
    let effective_password = if payload.password.trim().is_empty() {
        existing_profile
            .as_ref()
            .map(|profile| {
                decrypt_secret(&state.config.encryption_key, &profile.password_encrypted)
            })
            .transpose()?
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Enter your provider password when validating the profile for the first time."
                        .to_string(),
                )
            })?
    } else {
        payload.password.clone()
    };
    let credentials = XtreamCredentials {
        base_url: payload.base_url,
        username: payload.username,
        password: effective_password,
        output_format: payload.output_format,
    };
    let result = xtreme::validate_profile(&state.http_client, &credentials).await?;

    Ok(Json(ValidateProviderResponse {
        valid: result.valid,
        status: if result.valid { "valid" } else { "error" }.to_string(),
        message: result.message,
    }))
}

async fn save_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveProviderPayload>,
) -> ApiResult<ProviderProfileResponse> {
    let auth = require_auth(&state, &headers).await?;
    let epg_sources = normalize_epg_source_payloads(payload.epg_sources)?;
    let existing_profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_scheduled_sync_on, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?;
    let effective_password = if payload.password.trim().is_empty() {
        existing_profile
            .as_ref()
            .map(|profile| {
                decrypt_secret(&state.config.encryption_key, &profile.password_encrypted)
            })
            .transpose()?
            .ok_or_else(|| {
                AppError::BadRequest(
                    "Enter your provider password when saving the profile for the first time."
                        .to_string(),
                )
            })?
    } else {
        payload.password.clone()
    };
    let credentials = XtreamCredentials {
        base_url: payload.base_url.clone(),
        username: payload.username.clone(),
        password: effective_password.clone(),
        output_format: payload.output_format.clone(),
    };

    let validation = xtreme::validate_profile(&state.http_client, &credentials).await?;
    if !validation.valid {
        return Err(AppError::BadRequest(validation.message));
    }

    let encrypted_password = encrypt_secret(&state.config.encryption_key, &effective_password)?;
    let profile_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO provider_profiles (
          user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode, status, last_validated_at, last_sync_error
        )
        VALUES ($1, 'xtreme', $2, $3, $4, $5, $6, 'valid', NOW(), NULL)
        ON CONFLICT (user_id)
        DO UPDATE SET
          provider_type = 'xtreme',
          base_url = EXCLUDED.base_url,
          username = EXCLUDED.username,
          password_encrypted = EXCLUDED.password_encrypted,
          output_format = EXCLUDED.output_format,
          playback_mode = EXCLUDED.playback_mode,
          status = 'valid',
          last_validated_at = NOW(),
          last_sync_error = NULL,
          updated_at = NOW()
        RETURNING
          id
        "#,
    )
    .bind(auth.user_id)
    .bind(payload.base_url)
    .bind(payload.username)
    .bind(encrypted_password)
    .bind(payload.output_format)
    .bind(playback_mode_as_str(normalize_playback_mode(&payload.playback_mode)?))
    .fetch_one(&state.pool)
    .await?;

    store_epg_sources(&state.pool, profile_id, &epg_sources).await?;
    let provider = load_provider_profile_response(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Provider profile was not found after saving.".to_string())
        })?;

    Ok(Json(provider))
}

async fn trigger_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<SyncJobResponse> {
    let auth = require_auth(&state, &headers).await?;
    let profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_scheduled_sync_on, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Connect a provider before starting sync".to_string()))?;

    ensure_no_active_sync(&state.pool, profile.id).await?;
    let job = insert_sync_job(&state.pool, auth.user_id, profile.id, "full", "manual").await?;

    spawn_sync_job(state.clone(), auth.user_id, profile.id, job.id);
    Ok(Json(job))
}

async fn get_sync_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Option<SyncJobResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let job = sqlx::query_as::<_, SyncJobResponse>(
        r#"
        SELECT
          id,
          status,
          job_type,
          trigger,
          created_at,
          started_at,
          finished_at,
          current_phase,
          completed_phases,
          total_phases,
          phase_message,
          error_message
        FROM sync_jobs
        WHERE user_id = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(job))
}

fn spawn_periodic_sync_worker(state: AppState) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 5));
        loop {
            interval.tick().await;
            if let Err(error) = queue_daily_syncs(state.clone()).await {
                error!("periodic sync worker failed: {error:?}");
            }
        }
    })
}

async fn queue_daily_syncs(state: AppState) -> Result<()> {
    let today = Local::now().date_naive();
    let current_hour = Local::now().hour();
    if current_hour < state.config.daily_sync_hour_local {
        return Ok(());
    }

    let profiles = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_scheduled_sync_on, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE status = 'valid'
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    for profile in profiles {
        if profile.last_scheduled_sync_on == Some(today) {
            continue;
        }

        match ensure_no_active_sync(&state.pool, profile.id).await {
            Ok(()) => {}
            Err(AppError::BadRequest(_)) => continue,
            Err(other) => return Err(anyhow!("failed to inspect active syncs: {other:?}")),
        }

        sqlx::query(
            r#"
            UPDATE provider_profiles
            SET last_scheduled_sync_on = $2, updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(profile.id)
        .bind(today)
        .execute(&state.pool)
        .await?;

        let job = insert_sync_job(
            &state.pool,
            profile.user_id,
            profile.id,
            "full",
            "scheduled",
        )
        .await?;

        spawn_sync_job(state.clone(), profile.user_id, profile.id, job.id);
    }

    Ok(())
}

async fn ensure_no_active_sync(pool: &PgPool, profile_id: Uuid) -> Result<(), AppError> {
    let active_job_count = sqlx::query_scalar::<_, i64>(
        r#"SELECT COUNT(*) FROM sync_jobs WHERE profile_id = $1 AND status IN ('queued', 'running')"#,
    )
    .bind(profile_id)
    .fetch_one(pool)
    .await?;

    if active_job_count > 0 {
        return Err(AppError::BadRequest(
            "A sync is already queued or running for this provider.".to_string(),
        ));
    }

    Ok(())
}

async fn insert_sync_job(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_type: &str,
    trigger: &str,
) -> Result<SyncJobResponse> {
    let total_phases = total_phases_for_job(job_type);

    let job = sqlx::query_as::<_, SyncJobResponse>(
        r#"
        INSERT INTO sync_jobs (
          user_id,
          profile_id,
          status,
          job_type,
          trigger,
          current_phase,
          completed_phases,
          total_phases,
          phase_message
        )
        VALUES ($1, $2, 'queued', $3, $4, 'queued', 0, $5, 'Waiting to start')
        RETURNING
          id,
          status,
          job_type,
          trigger,
          created_at,
          started_at,
          finished_at,
          current_phase,
          completed_phases,
          total_phases,
          phase_message,
          error_message
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .bind(job_type)
    .bind(trigger)
    .bind(total_phases)
    .fetch_one(pool)
    .await?;

    Ok(job)
}

fn total_phases_for_job(job_type: &str) -> i32 {
    if job_type == "epg" {
        EPG_SYNC_TOTAL_PHASES
    } else {
        FULL_SYNC_TOTAL_PHASES
    }
}

async fn update_sync_job_phase(
    pool: &PgPool,
    job_id: Uuid,
    phase: &str,
    completed_phases: i32,
    job_type: &str,
    phase_message: &str,
) -> Result<()> {
    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET
          current_phase = $2,
          completed_phases = $3,
          total_phases = $4,
          phase_message = $5
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .bind(phase)
    .bind(completed_phases)
    .bind(total_phases_for_job(job_type))
    .bind(phase_message)
    .execute(pool)
    .await?;

    Ok(())
}

async fn list_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<ChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let mut channels = fetch_channels(&state.pool, auth.user_id).await?;
    rewrite_channel_logo_urls(&state, &headers, auth.user_id, &mut channels)?;
    Ok(Json(channels))
}

async fn get_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<ChannelResponse> {
    let auth = require_auth(&state, &headers).await?;
    let mut channel = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1 AND c.id = $2
        "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Channel not found".to_string()))?;
    channel.logo_url = rewrite_channel_logo_url(
        &state,
        &request_base_url(&state.config, &headers)?,
        auth.user_id,
        channel.profile_id,
        channel.logo_url,
    )?;

    Ok(Json(channel))
}

async fn get_guide(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<GuideResponse> {
    let auth = require_auth(&state, &headers).await?;
    let categories = fetch_guide_categories(&state.pool, auth.user_id).await?;
    Ok(Json(GuideResponse { categories }))
}

async fn get_guide_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<GuidePreferencesResponse> {
    let auth = require_auth(&state, &headers).await?;
    let included_category_ids = load_guide_preferences(&state.pool, auth.user_id).await?;

    Ok(Json(GuidePreferencesResponse {
        included_category_ids,
    }))
}

async fn save_guide_preferences(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveGuidePreferencesPayload>,
) -> ApiResult<GuidePreferencesResponse> {
    let auth = require_auth(&state, &headers).await?;
    let included_category_ids = normalize_category_ids(payload.included_category_ids);

    sqlx::query(
        r#"
        INSERT INTO user_guide_preferences (user_id, included_category_ids, updated_at)
        VALUES ($1, $2, NOW())
        ON CONFLICT (user_id)
        DO UPDATE SET
          included_category_ids = EXCLUDED.included_category_ids,
          updated_at = NOW()
        "#,
    )
    .bind(auth.user_id)
    .bind(&included_category_ids)
    .execute(&state.pool)
    .await?;

    Ok(Json(GuidePreferencesResponse {
        included_category_ids,
    }))
}

async fn get_guide_category(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(category_id): Path<String>,
    Query(query): Query<GuideCategoryQuery>,
) -> ApiResult<GuideCategoryResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (offset, limit) = parse_guide_category_pagination(query)?;
    let categories = fetch_guide_categories(&state.pool, auth.user_id).await?;
    let category = categories
        .into_iter()
        .find(|item| item.id == category_id)
        .ok_or_else(|| AppError::NotFound("Guide category not found".to_string()))?;
    let total_count =
        fetch_guide_category_total_count(&state.pool, auth.user_id, &category_id).await?;
    let rows =
        fetch_guide_category_rows(&state.pool, auth.user_id, &category_id, offset, limit).await?;
    let request_base_url = request_base_url(&state.config, &headers)?;
    let entries = rows
        .into_iter()
        .map(|row| map_guide_category_entry(&state, &request_base_url, auth.user_id, row))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(GuideCategoryResponse {
        category,
        next_offset: next_guide_offset(offset, limit, total_count),
        total_count,
        entries,
    }))
}

async fn get_channel_guide(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Vec<ProgramResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let programs = sqlx::query_as::<_, ProgramResponse>(
        r#"
        SELECT
          id,
          channel_id,
          channel_name,
          title,
          description,
          start_at,
          end_at,
          can_catchup
        FROM programs
        WHERE user_id = $1 AND channel_id = $2
        ORDER BY start_at ASC
        LIMIT 250
        "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(programs))
}

async fn search_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<ChannelSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (term, offset, limit) = parse_search_pagination(query)?;
    let total_count = count_channel_search_results(&state.pool, auth.user_id, &term).await?;
    let mut items = sqlx::query_as::<_, ChannelResponse>(
        r#"
        WITH page AS (
          SELECT ranked.entity_id, ROW_NUMBER() OVER () AS ordinal
          FROM (
            SELECT sd.entity_id
            FROM search_documents sd
            WHERE sd.user_id = $1
              AND sd.entity_type = 'channel'
              AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
            ORDER BY
              CASE WHEN lower(sd.title) = lower($2) THEN 0 ELSE 1 END,
              similarity(sd.search_text, $2) DESC,
              sd.title ASC
            OFFSET $3
            LIMIT $4
          ) ranked
        )
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite
        FROM page
        JOIN channels c ON c.id = page.entity_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        ORDER BY page.ordinal
        "#,
    )
    .bind(auth.user_id)
    .bind(&term)
    .bind(offset)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;
    rewrite_channel_logo_urls(&state, &headers, auth.user_id, &mut items)?;

    Ok(Json(ChannelSearchResponse {
        query: term,
        next_offset: next_page_offset(offset, limit, total_count),
        total_count,
        items,
    }))
}

async fn search_programs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SearchQuery>,
) -> ApiResult<ProgramSearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (term, offset, limit) = parse_search_pagination(query)?;
    let total_count = count_program_search_results(&state.pool, auth.user_id, &term).await?;
    let items = sqlx::query_as::<_, ProgramResponse>(
        r#"
        WITH page AS (
          SELECT ranked.id, ROW_NUMBER() OVER () AS ordinal
          FROM (
            SELECT p.id
            FROM search_documents sd
            JOIN programs p ON p.id = sd.entity_id
            WHERE sd.user_id = $1
              AND sd.entity_type = 'program'
              AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
            ORDER BY
              CASE
                WHEN p.channel_id IS NOT NULL AND sd.starts_at <= NOW() AND sd.ends_at >= NOW() THEN 0
                WHEN p.channel_id IS NOT NULL AND sd.ends_at <= NOW() AND p.can_catchup THEN 1
                WHEN lower(sd.title) = lower($2) THEN 2
                WHEN lower(sd.title) LIKE lower($2 || '%') THEN 3
                WHEN sd.starts_at > NOW() THEN 4
                ELSE 5
              END,
              similarity(sd.search_text, $2) DESC,
              sd.starts_at ASC
            OFFSET $3
            LIMIT $4
          ) ranked
        )
        SELECT
          p.id,
          p.channel_id,
          p.channel_name,
          p.title,
          p.description,
          p.start_at,
          p.end_at,
          p.can_catchup
        FROM page
        JOIN programs p ON p.id = page.id
        ORDER BY page.ordinal
        "#,
    )
    .bind(auth.user_id)
    .bind(&term)
    .bind(offset)
    .bind(limit)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(ProgramSearchResponse {
        query: term,
        next_offset: next_page_offset(offset, limit, total_count),
        total_count,
        items,
    }))
}

async fn list_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<ChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let mut favorites = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          TRUE AS is_favorite
        FROM favorites f
        JOIN channels c ON c.id = f.channel_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE f.user_id = $1
        ORDER BY f.created_at DESC
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;
    rewrite_channel_logo_urls(&state, &headers, auth.user_id, &mut favorites)?;

    Ok(Json(favorites))
}

async fn add_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query(
        r#"
        INSERT INTO favorites (user_id, channel_id)
        VALUES ($1, $2)
        ON CONFLICT (user_id, channel_id) DO NOTHING
        "#,
    )
    .bind(auth.user_id)
    .bind(channel_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_favorite(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(channel_id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query("DELETE FROM favorites WHERE user_id = $1 AND channel_id = $2")
        .bind(auth.user_id)
        .bind(channel_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_recents(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<RecentChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let rows = sqlx::query_as::<_, RecentChannelRow>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite,
          r.last_played_at
        FROM recents r
        JOIN channels c ON c.id = r.channel_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE r.user_id = $1
        ORDER BY r.last_played_at DESC
        LIMIT 20
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;
    let request_base_url = request_base_url(&state.config, &headers)?;

    let recents = rows
        .into_iter()
        .map(|row| {
            Ok(RecentChannelResponse {
                channel: ChannelResponse {
                    id: row.id,
                    profile_id: row.profile_id,
                    name: row.name,
                    logo_url: rewrite_channel_logo_url(
                        &state,
                        &request_base_url,
                        auth.user_id,
                        row.profile_id,
                        row.logo_url,
                    )?,
                    category_name: row.category_name,
                    remote_stream_id: row.remote_stream_id,
                    epg_channel_id: row.epg_channel_id,
                    has_catchup: row.has_catchup,
                    archive_duration_hours: row.archive_duration_hours,
                    stream_extension: row.stream_extension,
                    is_favorite: row.is_favorite,
                },
                last_played_at: row.last_played_at,
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;

    Ok(Json(recents))
}

async fn play_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    let record = sqlx::query_as::<_, ChannelPlaybackRecord>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.remote_stream_id,
          c.stream_extension,
          c.has_catchup,
          c.archive_duration_hours,
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
    .bind(auth.user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Channel not found".to_string()))?;

    let credentials = playback_credentials(&state, &record)?;
    let url = xtreme::build_live_stream_url(
        &credentials,
        record.remote_stream_id,
        record.stream_extension.as_deref(),
    )?;
    touch_recent(&state.pool, auth.user_id, record.id).await?;

    Ok(Json(playback_source_for_mode(
        &state,
        &headers,
        auth.user_id,
        record.profile_id,
        &record.playback_mode,
        &record.name,
        url,
        true,
        false,
        record.stream_extension.as_deref(),
        None,
    )?))
}

async fn play_program(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    let row = sqlx::query_as::<_, ProgramPlaybackRow>(
        r#"
        SELECT
          p.id,
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
    .bind(auth.user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Program not found".to_string()))?;

    let behavior = determine_program_playback_behavior(&row, Utc::now());

    let Some(channel_id) = row.channel_id else {
        return Ok(Json(unsupported_playback(
            &row.title,
            "This program is not mapped to a playable channel.",
        )));
    };
    touch_recent(&state.pool, auth.user_id, channel_id).await?;

    match behavior {
        ProgramPlaybackBehavior::Live => {
            let credentials = XtreamCredentials {
                base_url: row.base_url,
                username: row.provider_username,
                password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
                output_format: row.output_format,
            };
            let url = xtreme::build_live_stream_url(
                &credentials,
                row.remote_stream_id,
                row.stream_extension.as_deref(),
            )?;

            Ok(Json(playback_source_for_mode(
                &state,
                &headers,
                auth.user_id,
                row.profile_id,
                &row.playback_mode,
                &row.channel_name,
                url,
                true,
                false,
                row.stream_extension.as_deref(),
                None,
            )?))
        }
        ProgramPlaybackBehavior::Catchup => {
            let credentials = XtreamCredentials {
                base_url: row.base_url,
                username: row.provider_username,
                password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
                output_format: row.output_format,
            };
            let url = xtreme::build_catchup_url(
                &credentials,
                row.remote_stream_id,
                row.stream_extension.as_deref(),
                row.start_at,
                row.end_at,
            )?;

            Ok(Json(playback_source_for_mode(
                &state,
                &headers,
                auth.user_id,
                row.profile_id,
                &row.playback_mode,
                &row.title,
                url,
                false,
                true,
                row.stream_extension.as_deref(),
                None,
            )?))
        }
        ProgramPlaybackBehavior::Unsupported(reason) => {
            Ok(Json(unsupported_playback(&row.title, reason)))
        }
    }
}

async fn relay_hls_playlist(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<RelayTokenQuery>,
) -> Result<Response, AppError> {
    let relay = validate_relay_token(&state, &query.token, RelayAssetKind::Hls).await?;
    let public_base_url = request_base_url(&state.config, &headers)?;

    let response = state
        .http_client
        .get(relay.upstream_url.clone())
        .send()
        .await
        .map_err(|error| AppError::Internal(anyhow!(error)))?
        .error_for_status()
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    let response_url = response.url().clone();
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/vnd.apple.mpegurl"));
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    if bytes.len() > RELAY_PLAYLIST_MAX_BYTES {
        return Err(AppError::BadRequest(
            "The upstream playlist exceeded the relay size limit.".to_string(),
        ));
    }

    let manifest = String::from_utf8(bytes.to_vec()).map_err(|_| {
        AppError::BadRequest("The upstream playlist could not be decoded as UTF-8.".to_string())
    })?;
    let rewritten = rewrite_hls_manifest(
        &state,
        relay.user_id,
        relay.profile_id,
        relay.expires_at,
        &public_base_url,
        &response_url,
        &manifest,
    )?;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(rewritten))
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

async fn relay_raw_stream(
    State(state): State<AppState>,
    Query(query): Query<RelayTokenQuery>,
) -> Result<Response, AppError> {
    let relay = validate_relay_token(&state, &query.token, RelayAssetKind::Raw).await?;

    relay_stream_response(&state, relay.upstream_url).await
}

async fn relay_asset(
    State(state): State<AppState>,
    Query(query): Query<RelayTokenQuery>,
) -> Result<Response, AppError> {
    let relay = validate_relay_token(&state, &query.token, RelayAssetKind::Asset).await?;

    relay_stream_response(&state, relay.upstream_url).await
}

async fn relay_stream_response(state: &AppState, upstream_url: Url) -> Result<Response, AppError> {
    let response = state
        .http_client
        .get(upstream_url)
        .send()
        .await
        .map_err(|error| AppError::Internal(anyhow!(error)))?
        .error_for_status()
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    let content_type = response.headers().get(header::CONTENT_TYPE).cloned();
    let content_length = response.headers().get(header::CONTENT_LENGTH).cloned();
    let body = Body::from_stream(response.bytes_stream());

    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CACHE_CONTROL, "no-store");
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    if let Some(content_length) = content_length {
        builder = builder.header(header::CONTENT_LENGTH, content_length);
    }

    builder
        .body(body)
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

const GUIDE_DEFAULT_LIMIT: i64 = 40;
const GUIDE_MAX_LIMIT: i64 = 100;

async fn fetch_guide_categories(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<GuideCategorySummaryResponse>> {
    let rows = sqlx::query_as::<_, GuideCategorySummaryRow>(
        r#"
        SELECT
          COALESCE(c.category_id::text, 'uncategorized') AS id,
          COALESCE(cc.name, 'Uncategorized') AS name,
          COUNT(DISTINCT c.id) AS channel_count,
          COUNT(DISTINCT c.id) FILTER (
            WHERE p.start_at <= NOW() AND p.end_at > NOW()
          ) AS live_now_count
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN programs p
          ON p.user_id = c.user_id
         AND p.channel_id = c.id
         AND p.end_at > NOW() - INTERVAL '2 hours'
         AND p.start_at < NOW() + INTERVAL '6 hours'
        WHERE c.user_id = $1
        GROUP BY COALESCE(c.category_id::text, 'uncategorized'), COALESCE(cc.name, 'Uncategorized')
        ORDER BY live_now_count DESC, channel_count DESC, name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| GuideCategorySummaryResponse {
            id: row.id,
            name: row.name,
            channel_count: row.channel_count,
            live_now_count: row.live_now_count,
        })
        .collect())
}

async fn fetch_guide_category_total_count(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
) -> Result<i64> {
    let total_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM channels c
        WHERE c.user_id = $1
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .fetch_one(pool)
    .await?;

    Ok(total_count)
}

async fn fetch_guide_category_rows(
    pool: &PgPool,
    user_id: Uuid,
    category_id: &str,
    offset: i64,
    limit: i64,
) -> Result<Vec<GuideCategoryEntryRow>> {
    let rows = sqlx::query_as::<_, GuideCategoryEntryRow>(
        r#"
        SELECT
          c.id AS channel_id,
          c.profile_id,
          c.name AS channel_name,
          c.logo_url,
          cc.name AS category_name,
          c.remote_stream_id,
          c.epg_channel_id,
          c.has_catchup,
          c.archive_duration_hours,
          c.stream_extension,
          EXISTS(
            SELECT 1 FROM favorites f
            WHERE f.user_id = c.user_id AND f.channel_id = c.id
          ) AS is_favorite,
          p.id AS program_id,
          p.channel_id AS program_channel_id,
          p.channel_name AS program_channel_name,
          p.title AS program_title,
          p.description AS program_description,
          p.start_at AS program_start_at,
          p.end_at AS program_end_at,
          p.can_catchup AS program_can_catchup
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        LEFT JOIN LATERAL (
          SELECT
            p.id,
            p.channel_id,
            p.channel_name,
            p.title,
            p.description,
            p.start_at,
            p.end_at,
            p.can_catchup,
            (p.start_at <= NOW() AND p.end_at > NOW()) AS is_live
          FROM programs p
          WHERE p.user_id = c.user_id
            AND p.channel_id = c.id
            AND p.end_at > NOW() - INTERVAL '2 hours'
            AND p.start_at < NOW() + INTERVAL '6 hours'
          ORDER BY is_live DESC, p.start_at ASC, p.title ASC
          LIMIT 1
        ) p ON TRUE
        WHERE c.user_id = $1
          AND (
            ($2 = 'uncategorized' AND c.category_id IS NULL)
            OR c.category_id::text = $2
          )
        ORDER BY
          CASE
            WHEN p.start_at <= NOW() AND p.end_at > NOW() THEN 0
            WHEN p.start_at IS NOT NULL THEN 1
            ELSE 2
          END ASC,
          p.start_at ASC NULLS LAST,
          c.name ASC
        OFFSET $3
        LIMIT $4
        "#,
    )
    .bind(user_id)
    .bind(category_id)
    .bind(offset)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
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
    .fetch_all(pool)
    .await?;

    Ok(channels)
}

async fn load_guide_preferences(pool: &PgPool, user_id: Uuid) -> Result<Vec<String>> {
    let included_category_ids = sqlx::query_scalar::<_, Vec<String>>(
        r#"
        SELECT included_category_ids
        FROM user_guide_preferences
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(included_category_ids.unwrap_or_default())
}

fn parse_guide_category_pagination(query: GuideCategoryQuery) -> Result<(i64, i64), AppError> {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(GUIDE_DEFAULT_LIMIT);

    if offset < 0 {
        return Err(AppError::BadRequest(
            "Guide offset must be zero or greater".to_string(),
        ));
    }

    if limit <= 0 {
        return Err(AppError::BadRequest(
            "Guide limit must be greater than zero".to_string(),
        ));
    }

    Ok((offset, limit.min(GUIDE_MAX_LIMIT)))
}

fn parse_search_pagination(query: SearchQuery) -> Result<(String, i64, i64), AppError> {
    let term = query.q.trim().to_string();
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(SEARCH_DEFAULT_LIMIT);

    if term.len() < 2 {
        return Err(AppError::BadRequest(
            "Search query must be at least 2 characters".to_string(),
        ));
    }

    if offset < 0 {
        return Err(AppError::BadRequest(
            "Search offset must be zero or greater".to_string(),
        ));
    }

    if limit <= 0 {
        return Err(AppError::BadRequest(
            "Search limit must be greater than zero".to_string(),
        ));
    }

    Ok((term, offset, limit.min(SEARCH_MAX_LIMIT)))
}

async fn count_channel_search_results(pool: &PgPool, user_id: Uuid, query: &str) -> Result<i64> {
    let total_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM search_documents sd
        WHERE sd.user_id = $1
          AND sd.entity_type = 'channel'
          AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
        "#,
    )
    .bind(user_id)
    .bind(query)
    .fetch_one(pool)
    .await?;

    Ok(total_count)
}

async fn count_program_search_results(pool: &PgPool, user_id: Uuid, query: &str) -> Result<i64> {
    let total_count = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM search_documents sd
        WHERE sd.user_id = $1
          AND sd.entity_type = 'program'
          AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
        "#,
    )
    .bind(user_id)
    .bind(query)
    .fetch_one(pool)
    .await?;

    Ok(total_count)
}

fn next_page_offset(offset: i64, limit: i64, total_count: i64) -> Option<i64> {
    let next_offset = offset + limit;
    (next_offset < total_count).then_some(next_offset)
}

fn map_guide_category_entry(
    state: &AppState,
    request_base_url: &Url,
    user_id: Uuid,
    row: GuideCategoryEntryRow,
) -> Result<GuideChannelEntryResponse, AppError> {
    let program = map_guide_program_response(&row);

    Ok(GuideChannelEntryResponse {
        channel: ChannelResponse {
            id: row.channel_id,
            profile_id: row.profile_id,
            name: row.channel_name,
            logo_url: rewrite_channel_logo_url(
                state,
                request_base_url,
                user_id,
                row.profile_id,
                row.logo_url,
            )?,
            category_name: row.category_name,
            remote_stream_id: row.remote_stream_id,
            epg_channel_id: row.epg_channel_id,
            has_catchup: row.has_catchup,
            archive_duration_hours: row.archive_duration_hours,
            stream_extension: row.stream_extension,
            is_favorite: row.is_favorite,
        },
        program,
    })
}

fn map_guide_program_response(row: &GuideCategoryEntryRow) -> Option<ProgramResponse> {
    let id = row.program_id?;
    let Some(start_at) = row.program_start_at else {
        warn!("guide entry for program {id} is missing program_start_at; omitting program payload");
        return None;
    };
    let Some(end_at) = row.program_end_at else {
        warn!("guide entry for program {id} is missing program_end_at; omitting program payload");
        return None;
    };

    Some(ProgramResponse {
        id,
        channel_id: row.program_channel_id,
        channel_name: row.program_channel_name.clone(),
        title: row.program_title.clone().unwrap_or_default(),
        description: row.program_description.clone(),
        start_at,
        end_at,
        can_catchup: row.program_can_catchup.unwrap_or(false),
    })
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

fn next_guide_offset(offset: i64, limit: i64, total_count: i64) -> Option<i64> {
    let next_offset = offset + limit;
    (next_offset < total_count).then_some(next_offset)
}

fn normalize_category_ids(category_ids: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut normalized = Vec::with_capacity(category_ids.len());

    for category_id in category_ids {
        let category_id = category_id.trim();
        if category_id.is_empty() {
            continue;
        }

        if seen.insert(category_id.to_string()) {
            normalized.push(category_id.to_string());
        }
    }

    normalized
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

fn unsupported_playback(title: &str, reason: &str) -> PlaybackSourceResponse {
    PlaybackSourceResponse {
        kind: "unsupported".to_string(),
        url: String::new(),
        headers: HashMap::new(),
        live: false,
        catchup: false,
        expires_at: None,
        unsupported_reason: Some(reason.to_string()),
        title: title.to_string(),
    }
}

fn playback_source_for_mode(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    profile_id: Uuid,
    raw_playback_mode: &str,
    title: &str,
    upstream_url: String,
    live: bool,
    catchup: bool,
    extension: Option<&str>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<PlaybackSourceResponse, AppError> {
    let direct = playback_source_from_url(
        title,
        upstream_url.clone(),
        live,
        catchup,
        extension,
        expires_at,
    );
    let playback_mode = normalize_playback_mode(raw_playback_mode)?;
    if direct.kind == "unsupported" {
        return Ok(direct);
    }

    let request_base_url = request_base_url(&state.config, headers)?;
    let relay_required_for_https =
        should_force_relay_for_secure_request(&request_base_url, &upstream_url);
    if playback_mode == PlaybackMode::Direct && !relay_required_for_https {
        return Ok(direct);
    }

    let relay_kind = relay_asset_kind_for_extension(extension).ok_or_else(|| {
        AppError::BadRequest(
            "Relay mode only supports browser-playable stream formats.".to_string(),
        )
    })?;
    let relay_token =
        issue_relay_token(state, user_id, profile_id, &upstream_url, relay_kind, None)?;
    let relay_url = relay_url_for_token(&request_base_url, relay_kind, &relay_token.token)?;

    Ok(PlaybackSourceResponse {
        url: relay_url,
        expires_at: Some(relay_token.expires_at),
        ..direct
    })
}

fn should_force_relay_for_secure_request(request_base_url: &Url, upstream_url: &str) -> bool {
    if request_base_url.scheme() != "https" {
        return false;
    }

    Url::parse(upstream_url)
        .map(|url| url.scheme() == "http")
        .unwrap_or(false)
}

fn playback_source_from_url(
    title: &str,
    url: String,
    live: bool,
    catchup: bool,
    extension: Option<&str>,
    expires_at: Option<DateTime<Utc>>,
) -> PlaybackSourceResponse {
    let kind = match extension.unwrap_or("m3u8") {
        "m3u8" => "hls",
        "ts" => "mpegts",
        _ => "unsupported",
    };

    PlaybackSourceResponse {
        kind: kind.to_string(),
        url,
        headers: HashMap::new(),
        live,
        catchup,
        expires_at,
        unsupported_reason: (kind == "unsupported").then_some(
            "The provider returned a stream format Euripus v1 cannot play in-browser.".to_string(),
        ),
        title: title.to_string(),
    }
}

async fn validate_relay_token(
    state: &AppState,
    token: &str,
    expected_kind: RelayAssetKind,
) -> Result<ValidatedRelayToken, AppError> {
    let relay = decode_relay_token(&state.config, token, expected_kind)?;
    let valid_profile = sqlx::query_scalar::<_, i64>(
        r#"
        SELECT COUNT(*)
        FROM provider_profiles
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(relay.profile_id)
    .bind(relay.user_id)
    .fetch_one(&state.pool)
    .await?;
    if valid_profile == 0 {
        return Err(AppError::Unauthorized);
    }

    Ok(relay)
}

fn issue_relay_token(
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

fn decode_relay_token(
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

fn relay_asset_kind_for_extension(extension: Option<&str>) -> Option<RelayAssetKind> {
    match extension.unwrap_or("m3u8") {
        "m3u8" => Some(RelayAssetKind::Hls),
        "ts" => Some(RelayAssetKind::Raw),
        _ => None,
    }
}

fn relay_asset_kind_for_url(url: &Url) -> RelayAssetKind {
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

fn relay_url_for_token(
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

fn rewrite_hls_manifest(
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

struct ValidatedRelayToken {
    user_id: Uuid,
    profile_id: Uuid,
    upstream_url: Url,
    expires_at: DateTime<Utc>,
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

async fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<AuthContext, AppError> {
    let header_value = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = header_value
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;
    let claims = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;

    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AppError::Unauthorized)?;
    let session_id = Uuid::parse_str(&claims.sid).map_err(|_| AppError::Unauthorized)?;
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

    Ok(AuthContext {
        user_id,
        session_id,
    })
}

async fn create_session(
    state: &AppState,
    headers: &HeaderMap,
    user: &UserRecord,
) -> Result<IssuedSession, AppError> {
    let refresh_token = generate_refresh_token();
    let refresh_hash = hash_refresh_token(&refresh_token);
    let expires_at = Utc::now() + ChronoDuration::days(state.config.refresh_token_days);
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(ToString::to_string);

    let session = sqlx::query_as::<_, SessionRecord>(
        r#"
        INSERT INTO sessions (user_id, refresh_token_hash, user_agent, expires_at)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, refresh_token_hash, user_agent, created_at, expires_at, revoked_at, last_used_at
        "#,
    )
    .bind(user.id)
    .bind(refresh_hash)
    .bind(user_agent)
    .bind(expires_at)
    .fetch_one(&state.pool)
    .await?;

    issue_session(state, user, session.id, refresh_token)
}

async fn refresh_session_from_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<IssuedSession, AppError> {
    let session = get_valid_session_by_refresh_token(state, refresh_token).await?;
    let user = sqlx::query_as::<_, UserRecord>(
        r#"SELECT id, username, password_hash, created_at FROM users WHERE id = $1"#,
    )
    .bind(session.user_id)
    .fetch_one(&state.pool)
    .await?;

    let next_refresh_token = generate_refresh_token();
    let next_refresh_hash = hash_refresh_token(&next_refresh_token);
    sqlx::query(
        r#"
        UPDATE sessions
        SET refresh_token_hash = $1, last_used_at = NOW()
        WHERE id = $2
        "#,
    )
    .bind(next_refresh_hash)
    .bind(session.id)
    .execute(&state.pool)
    .await?;

    issue_session(state, &user, session.id, next_refresh_token)
}

async fn get_valid_session_by_refresh_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<SessionRecord, AppError> {
    let refresh_hash = hash_refresh_token(refresh_token);
    let session = sqlx::query_as::<_, SessionRecord>(
        r#"
        SELECT id, user_id, refresh_token_hash, user_agent, created_at, expires_at, revoked_at, last_used_at
        FROM sessions
        WHERE refresh_token_hash = $1
        "#,
    )
    .bind(&refresh_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::Unauthorized)?;

    if session.revoked_at.is_some() || session.expires_at < Utc::now() {
        return Err(AppError::Unauthorized);
    }

    Ok(session)
}

async fn revoke_session_by_refresh_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<(), AppError> {
    let refresh_hash = hash_refresh_token(refresh_token);
    sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE refresh_token_hash = $1")
        .bind(refresh_hash)
        .execute(&state.pool)
        .await?;

    Ok(())
}

fn issue_session(
    state: &AppState,
    user: &UserRecord,
    session_id: Uuid,
    refresh_token: String,
) -> Result<IssuedSession, AppError> {
    let (access_token, access_expires_at) = create_access_token(state, user, session_id)?;
    Ok(IssuedSession {
        session: AuthSessionResponse {
            user: UserResponse {
                id: user.id,
                username: user.username.clone(),
                created_at: user.created_at,
            },
            access_token,
            expires_at: access_expires_at,
        },
        refresh_token,
    })
}

fn desktop_auth_session_response(session: IssuedSession) -> DesktopAuthSessionResponse {
    DesktopAuthSessionResponse {
        session: session.session,
        refresh_token: session.refresh_token,
    }
}

fn browser_auth_response(
    state: &AppState,
    jar: CookieJar,
    issued_session: IssuedSession,
) -> (CookieJar, Json<AuthSessionResponse>) {
    let csrf_token = generate_refresh_token();
    let jar = set_browser_auth_cookies(state, jar, &issued_session.refresh_token, &csrf_token);
    (jar, Json(issued_session.session))
}

fn set_browser_auth_cookies(
    state: &AppState,
    jar: CookieJar,
    refresh_token: &str,
    csrf_token: &str,
) -> CookieJar {
    let refresh_cookie = build_refresh_cookie(state, refresh_token.to_string());
    let csrf_cookie = build_csrf_cookie(state, csrf_token.to_string());
    jar.add(refresh_cookie).add(csrf_cookie)
}

fn clear_browser_auth_cookies(state: &AppState, jar: CookieJar) -> CookieJar {
    jar.add(expired_refresh_cookie(state))
        .add(expired_csrf_cookie(state))
}

fn read_browser_refresh_token(jar: &CookieJar) -> Result<String, AppError> {
    jar.get(REFRESH_COOKIE_NAME)
        .map(|cookie| cookie.value().to_string())
        .ok_or(AppError::Unauthorized)
}

fn validate_browser_csrf(jar: &CookieJar, headers: &HeaderMap) -> Result<(), AppError> {
    let csrf_cookie = jar.get(CSRF_COOKIE_NAME).ok_or(AppError::Unauthorized)?;
    let csrf_header = headers
        .get(HeaderName::from_static(CSRF_HEADER_NAME))
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;

    if csrf_cookie.value() != csrf_header {
        return Err(AppError::Unauthorized);
    }

    Ok(())
}

fn build_refresh_cookie(state: &AppState, value: String) -> Cookie<'static> {
    let mut builder = Cookie::build((REFRESH_COOKIE_NAME, value))
        .http_only(true)
        .path("/api/auth")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::days(state.config.refresh_token_days));

    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }

    builder.build()
}

fn build_csrf_cookie(state: &AppState, value: String) -> Cookie<'static> {
    let mut builder = Cookie::build((CSRF_COOKIE_NAME, value))
        .http_only(false)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::days(state.config.refresh_token_days));

    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }

    builder.build()
}

fn expired_refresh_cookie(state: &AppState) -> Cookie<'static> {
    let mut builder = Cookie::build((REFRESH_COOKIE_NAME, ""))
        .http_only(true)
        .path("/api/auth")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(0));

    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }

    builder.build()
}

fn expired_csrf_cookie(state: &AppState) -> Cookie<'static> {
    let mut builder = Cookie::build((CSRF_COOKIE_NAME, ""))
        .http_only(false)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(0));

    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }

    builder.build()
}

fn create_access_token(
    state: &AppState,
    user: &UserRecord,
    session_id: Uuid,
) -> Result<(String, DateTime<Utc>)> {
    let expires_at = Utc::now() + ChronoDuration::minutes(state.config.access_token_minutes);
    let claims = AccessClaims {
        sub: user.id.to_string(),
        sid: session_id.to_string(),
        username: user.username.clone(),
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

fn spawn_sync_job(
    state: AppState,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(error) = run_sync_job(state.clone(), user_id, profile_id, job_id).await {
            error!("sync job {job_id} failed: {error:?}");
            let _ = sqlx::query(
                r#"
                UPDATE sync_jobs
                SET
                  status = 'failed',
                  finished_at = NOW(),
                  current_phase = 'failed',
                  phase_message = $2,
                  error_message = $2
                WHERE id = $1
                "#,
            )
            .bind(job_id)
            .bind(error.to_string())
            .execute(&state.pool)
            .await;

            let _ = sqlx::query(
                r#"
                UPDATE provider_profiles
                SET status = 'error', last_sync_error = $2, updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(profile_id)
            .bind(error.to_string())
            .execute(&state.pool)
            .await;
        }
    })
}

async fn run_sync_job(
    state: AppState,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
) -> Result<()> {
    sqlx::query(
        "UPDATE sync_jobs SET status = 'running', started_at = NOW(), current_phase = 'starting', phase_message = 'Preparing sync' WHERE id = $1",
    )
        .bind(job_id)
        .execute(&state.pool)
        .await?;
    sqlx::query(
        r#"UPDATE provider_profiles SET status = 'syncing', last_sync_error = NULL, updated_at = NOW() WHERE id = $1"#,
    )
    .bind(profile_id)
    .execute(&state.pool)
    .await?;
    let job_type = sqlx::query_scalar::<_, String>("SELECT job_type FROM sync_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&state.pool)
        .await?;

    let profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_scheduled_sync_on, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(profile_id)
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;

    let credentials = XtreamCredentials {
        base_url: profile.base_url.clone(),
        username: profile.username.clone(),
        password: decrypt_secret(&state.config.encryption_key, &profile.password_encrypted)?,
        output_format: profile.output_format.clone(),
    };

    update_sync_job_phase(
        &state.pool,
        job_id,
        "validating",
        0,
        &job_type,
        "Validating provider",
    )
    .await?;
    info!("sync job {job_id}: validating provider");
    let validation = xtreme::validate_profile(&state.http_client, &credentials).await?;
    if !validation.valid {
        return Err(anyhow!("provider validation failed during sync"));
    }

    let existing_channel_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM channels WHERE user_id = $1 AND profile_id = $2",
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_one(&state.pool)
    .await?;
    let refresh_channels = job_type == "full" || existing_channel_count == 0;

    let (categories, channels) = if refresh_channels {
        update_sync_job_phase(
            &state.pool,
            job_id,
            "fetching-categories",
            1,
            &job_type,
            "Fetching live categories",
        )
        .await?;
        info!("sync job {job_id}: fetching categories");
        let categories = xtreme::fetch_categories(&state.http_client, &credentials).await?;
        info!("sync job {job_id}: fetched {} categories", categories.len());
        update_sync_job_phase(
            &state.pool,
            job_id,
            "fetching-channels",
            2,
            &job_type,
            "Fetching live channels",
        )
        .await?;
        info!("sync job {job_id}: fetching live streams");
        let channels = xtreme::fetch_live_streams(&state.http_client, &credentials).await?;
        info!("sync job {job_id}: fetched {} live streams", channels.len());
        (Some(categories), Some(channels))
    } else {
        (None, None)
    };

    let epg_sources = sqlx::query_as::<_, EpgSourceRecord>(
        r#"
        SELECT
          id, profile_id, url, priority, enabled, source_kind, last_sync_at, last_sync_error,
          last_program_count, last_matched_count, created_at, updated_at
        FROM epg_sources
        WHERE profile_id = $1 AND enabled = TRUE
        ORDER BY priority ASC, created_at ASC
        "#,
    )
    .bind(profile_id)
    .fetch_all(&state.pool)
    .await?;
    let epg_fetch_completed_phases = if refresh_channels { 3 } else { 1 };
    update_sync_job_phase(
        &state.pool,
        job_id,
        "fetching-epg",
        epg_fetch_completed_phases,
        &job_type,
        "Fetching EPG feeds",
    )
    .await?;
    let (fetched_feeds, mut source_statuses) =
        fetch_epg_feeds(&state.http_client, &credentials, &epg_sources).await?;

    let epg_match_completed_phases = if refresh_channels { 4 } else { 2 };
    update_sync_job_phase(
        &state.pool,
        job_id,
        "matching-epg",
        epg_match_completed_phases,
        &job_type,
        "Matching guide data",
    )
    .await?;
    info!("sync job {job_id}: persisting sync data");
    let persisted_statuses = if refresh_channels {
        persist_full_sync_data(
            &state.pool,
            user_id,
            profile_id,
            job_id,
            &job_type,
            categories.as_deref().unwrap_or(&[]),
            channels.as_deref().unwrap_or(&[]),
            &fetched_feeds,
        )
        .await?
    } else {
        persist_epg_sync_data(
            &state.pool,
            user_id,
            profile_id,
            job_id,
            &job_type,
            &fetched_feeds,
        )
        .await?
    };
    source_statuses.extend(persisted_statuses);
    update_epg_source_statuses(&state.pool, &source_statuses).await?;
    info!("sync job {job_id}: finished persisting sync data");

    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET
          status = 'succeeded',
          finished_at = NOW(),
          current_phase = 'finished',
          completed_phases = total_phases,
          phase_message = 'Sync complete',
          error_message = NULL
        WHERE id = $1
        "#,
    )
    .bind(job_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        UPDATE provider_profiles
        SET status = 'valid', last_sync_at = NOW(), last_sync_error = NULL, last_validated_at = NOW(), updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(profile_id)
    .execute(&state.pool)
    .await?;

    Ok(())
}

async fn fetch_epg_feeds(
    client: &reqwest::Client,
    credentials: &XtreamCredentials,
    external_sources: &[EpgSourceRecord],
) -> Result<(Vec<FetchedEpgFeed>, Vec<EpgSourceSyncStatus>)> {
    let mut fetched_feeds = Vec::new();
    let mut source_statuses = Vec::new();
    let mut built_in_error = None;
    let mut join_set = JoinSet::new();
    let mut next_source_index = 0usize;

    while next_source_index < external_sources.len() && join_set.len() < EPG_FETCH_CONCURRENCY {
        let source = external_sources[next_source_index].clone();
        let client = client.clone();
        join_set.spawn(async move { fetch_external_epg_source(client, source).await });
        next_source_index += 1;
    }

    while let Some(result) = join_set.join_next().await {
        match result? {
            ExternalEpgFetchResult::Success(feed) => fetched_feeds.push(feed),
            ExternalEpgFetchResult::Failure(status) => source_statuses.push(status),
        }

        if next_source_index < external_sources.len() {
            let source = external_sources[next_source_index].clone();
            let client = client.clone();
            join_set.spawn(async move { fetch_external_epg_source(client, source).await });
            next_source_index += 1;
        }
    }

    match xtreme::fetch_xmltv(client, credentials).await {
        Ok(feed) => fetched_feeds.push(FetchedEpgFeed {
            source_id: None,
            source_kind: "xtream".to_string(),
            source_label: xtreme::build_xmltv_url(credentials)?.to_string(),
            priority: external_sources
                .iter()
                .map(|source| source.priority)
                .max()
                .unwrap_or(-1)
                + 1,
            feed,
        }),
        Err(error) => {
            built_in_error = Some(error.to_string());
            error!("failed to fetch built-in Xtream XMLTV feed: {error:?}");
        }
    }

    fetched_feeds.sort_by_key(|feed| feed.priority);

    if fetched_feeds.is_empty() {
        return Err(anyhow!(
            "no EPG feed could be ingested: {}",
            built_in_error.unwrap_or_else(|| "All configured EPG sources failed.".to_string())
        ));
    }

    Ok((fetched_feeds, source_statuses))
}

async fn fetch_external_epg_source(
    client: reqwest::Client,
    source: EpgSourceRecord,
) -> ExternalEpgFetchResult {
    match url::Url::parse(&source.url) {
        Ok(url) => match xmltv::fetch_xmltv(&client, &url).await {
            Ok(feed) => ExternalEpgFetchResult::Success(FetchedEpgFeed {
                source_id: Some(source.id),
                source_kind: source.source_kind,
                source_label: source.url,
                priority: source.priority,
                feed,
            }),
            Err(error) => {
                error!(
                    "failed to fetch external EPG source {}: {error:?}",
                    source.url
                );
                ExternalEpgFetchResult::Failure(EpgSourceSyncStatus {
                    source_id: source.id,
                    last_sync_error: Some(error.to_string()),
                    last_program_count: None,
                    last_matched_count: None,
                    mark_synced: false,
                })
            }
        },
        Err(error) => ExternalEpgFetchResult::Failure(EpgSourceSyncStatus {
            source_id: source.id,
            last_sync_error: Some(format!("Invalid EPG source URL: {error}")),
            last_program_count: None,
            last_matched_count: None,
            mark_synced: false,
        }),
    }
}

async fn persist_full_sync_data(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
    job_type: &str,
    categories: &[XtreamCategory],
    channels: &[XtreamChannel],
    feeds: &[FetchedEpgFeed],
) -> Result<Vec<EpgSourceSyncStatus>> {
    let mut transaction = pool.begin().await?;
    bulk_upsert_categories(&mut transaction, user_id, profile_id, categories).await?;
    bulk_upsert_channels(&mut transaction, user_id, profile_id, channels).await?;
    let persisted_channels = load_persisted_channels(&mut transaction, user_id, profile_id).await?;
    let channel_lookup = build_channel_lookup_index(&persisted_channels);
    let (programmes, source_statuses) = resolve_epg_programmes(feeds, &channel_lookup);

    update_sync_job_phase(
        pool,
        job_id,
        "saving-programs",
        5,
        job_type,
        "Saving guide entries",
    )
    .await?;
    sqlx::query("DELETE FROM programs WHERE user_id = $1 AND profile_id = $2")
        .bind(user_id)
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
    bulk_insert_programmes(&mut transaction, user_id, profile_id, &programmes).await?;

    update_sync_job_phase(
        pool,
        job_id,
        "rebuilding-search",
        6,
        job_type,
        "Refreshing search",
    )
    .await?;
    rebuild_search_documents(&mut transaction, user_id).await?;

    transaction.commit().await?;
    Ok(source_statuses)
}

async fn persist_epg_sync_data(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    job_id: Uuid,
    job_type: &str,
    feeds: &[FetchedEpgFeed],
) -> Result<Vec<EpgSourceSyncStatus>> {
    let mut transaction = pool.begin().await?;
    let persisted_channels = load_persisted_channels(&mut transaction, user_id, profile_id).await?;
    let channel_lookup = build_channel_lookup_index(&persisted_channels);
    let (programmes, source_statuses) = resolve_epg_programmes(feeds, &channel_lookup);

    update_sync_job_phase(
        pool,
        job_id,
        "saving-programs",
        3,
        job_type,
        "Saving guide entries",
    )
    .await?;
    sqlx::query("DELETE FROM programs WHERE user_id = $1 AND profile_id = $2")
        .bind(user_id)
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;
    bulk_insert_programmes(&mut transaction, user_id, profile_id, &programmes).await?;

    update_sync_job_phase(
        pool,
        job_id,
        "rebuilding-search",
        4,
        job_type,
        "Refreshing search",
    )
    .await?;
    rebuild_search_documents(&mut transaction, user_id).await?;

    transaction.commit().await?;
    Ok(source_statuses)
}

async fn update_epg_source_statuses(pool: &PgPool, statuses: &[EpgSourceSyncStatus]) -> Result<()> {
    for status in statuses {
        sqlx::query(
            r#"
            UPDATE epg_sources
            SET
              last_sync_at = CASE WHEN $2 THEN NOW() ELSE last_sync_at END,
              last_sync_error = $3,
              last_program_count = $4,
              last_matched_count = $5,
              updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(status.source_id)
        .bind(status.mark_synced)
        .bind(&status.last_sync_error)
        .bind(status.last_program_count)
        .bind(status.last_matched_count)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn bulk_upsert_categories(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    categories: &[XtreamCategory],
) -> Result<()> {
    let deduped_categories = categories
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut categories_by_remote_id, category| {
            categories_by_remote_id.insert(category.remote_category_id.clone(), category);
            categories_by_remote_id
        })
        .into_values()
        .collect::<Vec<_>>();

    for chunk in deduped_categories.chunks(SYNC_BATCH_SIZE) {
        let remote_category_ids = chunk
            .iter()
            .map(|category| category.remote_category_id.clone())
            .collect::<Vec<_>>();
        let names = chunk
            .iter()
            .map(|category| category.name.clone())
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST($3::text[], $4::text[]) AS input(remote_category_id, name)
            )
            INSERT INTO channel_categories (user_id, profile_id, remote_category_id, name)
            SELECT $1, $2, input.remote_category_id, input.name
            FROM input
            ON CONFLICT (user_id, profile_id, remote_category_id)
            DO UPDATE SET name = EXCLUDED.name
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&remote_category_ids)
        .bind(&names)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn bulk_upsert_channels(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    channels: &[XtreamChannel],
) -> Result<()> {
    let deduped_channels = channels
        .iter()
        .cloned()
        .fold(HashMap::new(), |mut channels_by_stream_id, channel| {
            channels_by_stream_id.insert(channel.remote_stream_id, channel);
            channels_by_stream_id
        })
        .into_values()
        .collect::<Vec<_>>();

    for chunk in deduped_channels.chunks(SYNC_BATCH_SIZE) {
        let remote_stream_ids = chunk
            .iter()
            .map(|channel| channel.remote_stream_id)
            .collect::<Vec<_>>();
        let names = chunk
            .iter()
            .map(|channel| channel.name.clone())
            .collect::<Vec<_>>();
        let logo_urls = chunk
            .iter()
            .map(|channel| channel.logo_url.clone())
            .collect::<Vec<_>>();
        let category_remote_ids = chunk
            .iter()
            .map(|channel| channel.category_id.clone())
            .collect::<Vec<_>>();
        let has_catchup = chunk
            .iter()
            .map(|channel| channel.has_catchup)
            .collect::<Vec<_>>();
        let archive_duration_hours = chunk
            .iter()
            .map(|channel| channel.archive_duration_hours)
            .collect::<Vec<_>>();
        let stream_extensions = chunk
            .iter()
            .map(|channel| channel.stream_extension.clone())
            .collect::<Vec<_>>();
        let epg_channel_ids = chunk
            .iter()
            .map(|channel| channel.epg_channel_id.clone())
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST(
                $3::int4[],
                $4::text[],
                $5::text[],
                $6::text[],
                $7::bool[],
                $8::int4[],
                $9::text[],
                $10::text[]
              ) AS input(
                remote_stream_id,
                name,
                logo_url,
                category_remote_id,
                has_catchup,
                archive_duration_hours,
                stream_extension,
                epg_channel_id
              )
            )
            INSERT INTO channels (
              user_id,
              profile_id,
              category_id,
              remote_stream_id,
              epg_channel_id,
              name,
              logo_url,
              has_catchup,
              archive_duration_hours,
              stream_extension,
              updated_at
            )
            SELECT
              $1,
              $2,
              cc.id,
              input.remote_stream_id,
              input.epg_channel_id,
              input.name,
              input.logo_url,
              input.has_catchup,
              input.archive_duration_hours,
              input.stream_extension,
              NOW()
            FROM input
            LEFT JOIN channel_categories cc
              ON cc.user_id = $1
             AND cc.profile_id = $2
             AND cc.remote_category_id = input.category_remote_id
            ON CONFLICT (user_id, profile_id, remote_stream_id)
            DO UPDATE SET
              category_id = EXCLUDED.category_id,
              epg_channel_id = EXCLUDED.epg_channel_id,
              name = EXCLUDED.name,
              logo_url = EXCLUDED.logo_url,
              has_catchup = EXCLUDED.has_catchup,
              archive_duration_hours = EXCLUDED.archive_duration_hours,
              stream_extension = EXCLUDED.stream_extension,
              updated_at = NOW()
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&remote_stream_ids)
        .bind(&names)
        .bind(&logo_urls)
        .bind(&category_remote_ids)
        .bind(&has_catchup)
        .bind(&archive_duration_hours)
        .bind(&stream_extensions)
        .bind(&epg_channel_ids)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn load_persisted_channels(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<Vec<PersistedChannelRecord>> {
    let channels = sqlx::query_as::<_, PersistedChannelRecord>(
        r#"
        SELECT
          id,
          name,
          remote_stream_id,
          epg_channel_id,
          has_catchup,
          updated_at
        FROM channels
        WHERE user_id = $1 AND profile_id = $2
        ORDER BY updated_at DESC, id DESC
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_all(&mut **transaction)
    .await?;

    Ok(channels)
}

fn build_channel_lookup_index(channels: &[PersistedChannelRecord]) -> ChannelLookupIndex {
    let mut lookup = ChannelLookupIndex::default();
    let mut ambiguous_simplified_names = HashSet::new();

    for channel in channels {
        let resolution = ChannelResolution {
            channel_id: channel.id,
            channel_name: channel.name.clone(),
            has_catchup: channel.has_catchup,
        };
        if let Some(epg_channel_id) = channel
            .epg_channel_id
            .as_ref()
            .filter(|value| !value.is_empty())
        {
            lookup
                .epg_channel_ids
                .entry(epg_channel_id.clone())
                .or_insert_with(|| resolution.clone());
        }
        lookup
            .remote_stream_ids
            .entry(channel.remote_stream_id.to_string())
            .or_insert_with(|| resolution.clone());
        let normalized_name = normalize_channel_name(&channel.name);
        if !normalized_name.is_empty() {
            lookup
                .normalized_names
                .entry(normalized_name)
                .or_insert_with(|| resolution.clone());
        }
        let simplified_name = simplify_channel_name(&channel.name);
        if !simplified_name.is_empty() {
            insert_unique_channel_alias(
                &mut lookup.simplified_names,
                &mut ambiguous_simplified_names,
                simplified_name,
                resolution,
            );
        }
    }

    lookup
}

fn normalize_channel_name(value: &str) -> String {
    channel_name_tokens(value).join("")
}

fn simplify_channel_name(value: &str) -> String {
    channel_name_tokens(value)
        .into_iter()
        .filter(|token| !is_channel_noise_token(token))
        .collect::<Vec<_>>()
        .join("")
}

fn channel_name_tokens(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in normalize_channel_text(value)
        .chars()
        .flat_map(|character| character.to_lowercase())
    {
        if character.is_alphanumeric() {
            current.push(character);
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    trim_channel_region_tokens(tokens)
}

fn normalize_channel_text(value: &str) -> String {
    value
        .replace("ᵁᴴᴰ", "UHD")
        .replace("ᶠᴴᴰ", "FHD")
        .replace("ᴴᴰ", "HD")
        .replace("ˢᴰ", "SD")
        .replace("⁴ᴷ", "4K")
}

fn trim_channel_region_tokens(mut tokens: Vec<String>) -> Vec<String> {
    while tokens
        .first()
        .map(|token| is_channel_region_token(token))
        .unwrap_or(false)
    {
        tokens.remove(0);
    }

    while tokens
        .last()
        .map(|token| is_channel_region_token(token))
        .unwrap_or(false)
    {
        tokens.pop();
    }

    tokens
}

fn is_channel_region_token(token: &str) -> bool {
    matches!(token, "se" | "swe" | "sweden")
}

fn is_channel_noise_token(token: &str) -> bool {
    matches!(
        token,
        "hd" | "uhd" | "fhd" | "sd" | "4k" | "text" | "multi" | "sub" | "audio" | "dub" | "dubbed"
    )
}

fn insert_unique_channel_alias(
    aliases: &mut HashMap<String, ChannelResolution>,
    ambiguous_aliases: &mut HashSet<String>,
    alias: String,
    resolution: ChannelResolution,
) {
    if ambiguous_aliases.contains(&alias) {
        return;
    }

    match aliases.get(&alias) {
        None => {
            aliases.insert(alias, resolution);
        }
        Some(existing) if existing.channel_id == resolution.channel_id => {}
        Some(_) => {
            aliases.remove(&alias);
            ambiguous_aliases.insert(alias);
        }
    }
}

fn resolve_channel_for_programme(
    programme: &XmltvProgramme,
    channels: &HashMap<String, XmltvChannel>,
    lookup: &ChannelLookupIndex,
) -> Option<ChannelResolution> {
    if let Some(channel) = lookup.epg_channel_ids.get(&programme.channel_key) {
        return Some(channel.clone());
    }

    if let Some(channel) = lookup.remote_stream_ids.get(&programme.channel_key) {
        return Some(channel.clone());
    }

    let display_names = channels
        .get(&programme.channel_key)
        .map(|channel| channel.display_names.as_slice())
        .unwrap_or(&[]);
    for display_name in display_names {
        let normalized_name = normalize_channel_name(display_name);
        if normalized_name.is_empty() {
            continue;
        }

        if let Some(channel) = lookup.normalized_names.get(&normalized_name) {
            return Some(channel.clone());
        }

        let simplified_name = simplify_channel_name(display_name);
        if simplified_name.is_empty() {
            continue;
        }

        if let Some(channel) = lookup.simplified_names.get(&simplified_name) {
            return Some(channel.clone());
        }
    }

    None
}

fn resolve_epg_programmes(
    feeds: &[FetchedEpgFeed],
    lookup: &ChannelLookupIndex,
) -> (Vec<ResolvedProgramme>, Vec<EpgSourceSyncStatus>) {
    let mut selected_slots = HashSet::new();
    let mut resolved_programmes = Vec::new();
    let mut source_statuses = Vec::new();

    for feed in feeds {
        let mut matched_count = 0i32;
        for programme in &feed.feed.programmes {
            let Some(channel) =
                resolve_channel_for_programme(programme, &feed.feed.channels, lookup)
            else {
                continue;
            };
            matched_count += 1;

            let slot_key = (
                channel.channel_id,
                programme.start_at.timestamp(),
                programme.end_at.timestamp(),
            );
            if !selected_slots.insert(slot_key) {
                continue;
            }

            resolved_programmes.push(ResolvedProgramme {
                channel_id: channel.channel_id,
                channel_name: channel.channel_name,
                title: programme.title.clone(),
                description: programme.description.clone(),
                start_at: programme.start_at,
                end_at: programme.end_at,
                can_catchup: channel.has_catchup,
            });
        }

        if let Some(source_id) = feed.source_id {
            source_statuses.push(EpgSourceSyncStatus {
                source_id,
                last_sync_error: None,
                last_program_count: Some(feed.feed.programmes.len() as i32),
                last_matched_count: Some(matched_count),
                mark_synced: true,
            });
        }
    }

    resolved_programmes.sort_by_key(|programme| {
        (
            programme.channel_name.clone(),
            programme.start_at.timestamp(),
            programme.end_at.timestamp(),
        )
    });

    (resolved_programmes, source_statuses)
}

async fn bulk_insert_programmes(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    programmes: &[ResolvedProgramme],
) -> Result<()> {
    for chunk in programmes.chunks(SYNC_BATCH_SIZE) {
        let channel_ids = chunk
            .iter()
            .map(|programme| programme.channel_id)
            .collect::<Vec<_>>();
        let channel_names = chunk
            .iter()
            .map(|programme| programme.channel_name.clone())
            .collect::<Vec<_>>();
        let titles = chunk
            .iter()
            .map(|programme| programme.title.clone())
            .collect::<Vec<_>>();
        let descriptions = chunk
            .iter()
            .map(|programme| programme.description.clone())
            .collect::<Vec<_>>();
        let start_times = chunk
            .iter()
            .map(|programme| programme.start_at)
            .collect::<Vec<_>>();
        let end_times = chunk
            .iter()
            .map(|programme| programme.end_at)
            .collect::<Vec<_>>();
        let can_catchup = chunk
            .iter()
            .map(|programme| programme.can_catchup)
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST(
                $3::uuid[],
                $4::text[],
                $5::text[],
                $6::text[],
                $7::timestamptz[],
                $8::timestamptz[],
                $9::bool[]
              ) AS input(channel_id, channel_name, title, description, start_at, end_at, can_catchup)
            )
            INSERT INTO programs (
              user_id,
              profile_id,
              channel_id,
              channel_name,
              title,
              description,
              start_at,
              end_at,
              can_catchup
            )
            SELECT
              $1,
              $2,
              input.channel_id,
              input.channel_name,
              input.title,
              input.description,
              input.start_at,
              input.end_at,
              input.can_catchup
            FROM input
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&channel_ids)
        .bind(&channel_names)
        .bind(&titles)
        .bind(&descriptions)
        .bind(&start_times)
        .bind(&end_times)
        .bind(&can_catchup)
        .execute(&mut **transaction)
        .await?;
    }

    Ok(())
}

async fn rebuild_search_documents(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"
        DELETE FROM search_documents
        WHERE user_id = $1
          AND entity_type IN ('channel', 'program')
        "#,
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO search_documents (user_id, entity_type, entity_id, title, subtitle, search_text, starts_at, ends_at)
        SELECT
          $1,
          'channel',
          c.id,
          c.name,
          cc.name,
          concat_ws(
            ' ',
            c.name,
            cc.name,
            CASE WHEN c.has_catchup THEN 'catchup archive' ELSE 'live' END
          ),
          NULL,
          NULL
        FROM channels c
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE c.user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO search_documents (user_id, entity_type, entity_id, title, subtitle, search_text, starts_at, ends_at)
        SELECT
          $1,
          'program',
          p.id,
          p.title,
          p.channel_name,
          concat_ws(' ', p.title, p.channel_name, p.description),
          p.start_at,
          p.end_at
        FROM programs p
        WHERE p.user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

#[derive(Debug, FromRow)]
struct RecentChannelRow {
    id: Uuid,
    profile_id: Uuid,
    name: String,
    logo_url: Option<String>,
    category_name: Option<String>,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
    is_favorite: bool,
    last_played_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ProgramPlaybackRow {
    id: Uuid,
    title: String,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    can_catchup: bool,
    profile_id: Uuid,
    channel_id: Option<Uuid>,
    remote_stream_id: i32,
    stream_extension: Option<String>,
    channel_name: String,
    has_catchup: bool,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
    playback_mode: String,
}

#[derive(Debug, PartialEq, Eq)]
enum ProgramPlaybackBehavior {
    Live,
    Catchup,
    Unsupported(&'static str),
}

fn determine_program_playback_behavior(
    row: &ProgramPlaybackRow,
    now: DateTime<Utc>,
) -> ProgramPlaybackBehavior {
    if row.channel_id.is_none() {
        return ProgramPlaybackBehavior::Unsupported(
            "This program is not mapped to a playable channel.",
        );
    }

    if row.start_at <= now && row.end_at > now {
        return ProgramPlaybackBehavior::Live;
    }

    if row.end_at <= now && row.can_catchup && row.has_catchup {
        return ProgramPlaybackBehavior::Catchup;
    }

    ProgramPlaybackBehavior::Unsupported(
        "Catch-up is not available for this program on the provider.",
    )
}

#[derive(Debug, FromRow)]
struct GuideCategorySummaryRow {
    id: String,
    name: String,
    channel_count: i64,
    live_now_count: i64,
}

#[derive(Debug, FromRow)]
struct GuideCategoryEntryRow {
    channel_id: Uuid,
    profile_id: Uuid,
    channel_name: String,
    logo_url: Option<String>,
    category_name: Option<String>,
    remote_stream_id: i32,
    epg_channel_id: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    stream_extension: Option<String>,
    is_favorite: bool,
    program_id: Option<Uuid>,
    program_channel_id: Option<Uuid>,
    program_channel_name: Option<String>,
    program_title: Option<String>,
    program_description: Option<String>,
    program_start_at: Option<DateTime<Utc>>,
    program_end_at: Option<DateTime<Utc>>,
    program_can_catchup: Option<bool>,
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
    fn produces_hls_kind_for_m3u8_urls() {
        let response = playback_source_from_url(
            "News",
            "https://example.com/live.m3u8".to_string(),
            true,
            false,
            Some("m3u8"),
            None,
        );
        assert_eq!(response.kind, "hls");
    }

    #[tokio::test]
    async fn playback_source_for_mode_keeps_direct_urls_in_direct_mode() {
        let state = sample_app_state();
        let response = playback_source_for_mode(
            &state,
            &HeaderMap::new(),
            Uuid::from_u128(1),
            Uuid::from_u128(2),
            "direct",
            "Arena 1",
            "https://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            Some("m3u8"),
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
            "relay",
            "Arena 1",
            "https://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            Some("m3u8"),
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
            "direct",
            "Arena 1",
            "http://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            Some("m3u8"),
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
            "direct",
            "Arena 1",
            "http://provider.example.com/live/42.m3u8".to_string(),
            true,
            false,
            Some("m3u8"),
            None,
        )
        .expect("direct playback source");

        assert_eq!(response.kind, "hls");
        assert_eq!(response.url, "http://provider.example.com/live/42.m3u8");
        assert!(response.expires_at.is_none());
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

        let relay = decode_relay_token(
            &state.config,
            &extract_relay_token(&logo_url),
            RelayAssetKind::Asset,
        )
        .expect("decode relay token");
        assert_eq!(
            relay.upstream_url.as_str(),
            "http://provider.example.com/logo.png"
        );
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
    fn parses_guide_category_pagination_defaults_and_caps_limit() {
        let (offset, limit) = parse_guide_category_pagination(GuideCategoryQuery {
            offset: None,
            limit: Some(GUIDE_MAX_LIMIT + 25),
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

    #[test]
    fn program_playback_uses_live_channel_when_program_is_airing() {
        let now = Utc::now();
        let row = sample_program_playback_row(
            now - ChronoDuration::minutes(15),
            now + ChronoDuration::minutes(45),
        );

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(behavior, ProgramPlaybackBehavior::Live);
    }

    #[test]
    fn program_playback_uses_catchup_when_program_has_ended_and_archive_is_available() {
        let now = Utc::now();
        let row = sample_program_playback_row(
            now - ChronoDuration::hours(2),
            now - ChronoDuration::hours(1),
        );

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(behavior, ProgramPlaybackBehavior::Catchup);
    }

    #[test]
    fn program_playback_is_unsupported_for_upcoming_programs() {
        let now = Utc::now();
        let row = sample_program_playback_row(
            now + ChronoDuration::minutes(10),
            now + ChronoDuration::minutes(70),
        );

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(
            behavior,
            ProgramPlaybackBehavior::Unsupported(
                "Catch-up is not available for this program on the provider.",
            )
        );
    }

    #[test]
    fn program_playback_is_unsupported_when_program_is_not_mapped_to_a_channel() {
        let now = Utc::now();
        let mut row = sample_program_playback_row(
            now - ChronoDuration::minutes(15),
            now + ChronoDuration::minutes(45),
        );
        row.channel_id = None;

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(
            behavior,
            ProgramPlaybackBehavior::Unsupported(
                "This program is not mapped to a playable channel.",
            )
        );
    }

    #[test]
    fn resolves_external_epg_programmes_by_xmltv_display_name() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(11),
            name: "TV4 HD".to_string(),
            remote_stream_id: 4,
            epg_channel_id: None,
            has_catchup: true,
            updated_at: now,
        }]);
        let feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(12)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/tv.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::from([(
                    "external-tv4".to_string(),
                    XmltvChannel {
                        id: "external-tv4".to_string(),
                        display_names: vec!["TV4 HD".to_string()],
                    },
                )]),
                programmes: vec![XmltvProgramme {
                    channel_key: "external-tv4".to_string(),
                    title: "Morning Show".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(1),
                }],
            },
        };

        let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_name, "TV4 HD");
        assert_eq!(programmes[0].title, "Morning Show");
        assert_eq!(statuses[0].last_matched_count, Some(1));
    }

    #[test]
    fn resolves_external_epg_programmes_with_region_and_quality_decorations() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(13),
            name: "|SE|TV4 ᴴᴰ SE".to_string(),
            remote_stream_id: 41,
            epg_channel_id: None,
            has_catchup: true,
            updated_at: now,
        }]);
        let feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(14)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/tv4.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::from([(
                    "tv4.se".to_string(),
                    XmltvChannel {
                        id: "tv4.se".to_string(),
                        display_names: vec!["TV4 HD.se".to_string()],
                    },
                )]),
                programmes: vec![XmltvProgramme {
                    channel_key: "tv4.se".to_string(),
                    title: "Evening News".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(1),
                }],
            },
        };

        let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_name, "|SE|TV4 ᴴᴰ SE");
        assert_eq!(programmes[0].title, "Evening News");
        assert_eq!(statuses[0].last_matched_count, Some(1));
    }

    #[test]
    fn resolves_external_epg_programmes_when_feed_uses_text_variant_names() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(15),
            name: "|SE|TV4 FAKTA".to_string(),
            remote_stream_id: 42,
            epg_channel_id: None,
            has_catchup: false,
            updated_at: now,
        }]);
        let feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(16)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/tv4fakta.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::from([(
                    "tv4-fakta.se".to_string(),
                    XmltvChannel {
                        id: "tv4-fakta.se".to_string(),
                        display_names: vec!["TV4 Fakta - Text.se".to_string()],
                    },
                )]),
                programmes: vec![XmltvProgramme {
                    channel_key: "tv4-fakta.se".to_string(),
                    title: "Documentary Hour".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(1),
                }],
            },
        };

        let (programmes, statuses) = resolve_epg_programmes(&[feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_name, "|SE|TV4 FAKTA");
        assert_eq!(programmes[0].title, "Documentary Hour");
        assert_eq!(statuses[0].last_matched_count, Some(1));
    }

    #[test]
    fn keeps_higher_priority_epg_source_when_timeslots_overlap() {
        let now = Utc::now();
        let lookup = build_channel_lookup_index(&[PersistedChannelRecord {
            id: Uuid::from_u128(21),
            name: "Arena 1".to_string(),
            remote_stream_id: 1,
            epg_channel_id: Some("arena.1".to_string()),
            has_catchup: true,
            updated_at: now,
        }]);
        let primary_feed = FetchedEpgFeed {
            source_id: Some(Uuid::from_u128(22)),
            source_kind: "external".to_string(),
            source_label: "https://example.com/primary.xml.gz".to_string(),
            priority: 0,
            feed: XmltvFeed {
                channels: HashMap::new(),
                programmes: vec![XmltvProgramme {
                    channel_key: "arena.1".to_string(),
                    title: "Primary Listing".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(2),
                }],
            },
        };
        let fallback_feed = FetchedEpgFeed {
            source_id: None,
            source_kind: "xtream".to_string(),
            source_label: "https://provider.example.com/xmltv.php".to_string(),
            priority: 1,
            feed: XmltvFeed {
                channels: HashMap::new(),
                programmes: vec![XmltvProgramme {
                    channel_key: "arena.1".to_string(),
                    title: "Fallback Listing".to_string(),
                    description: None,
                    start_at: now,
                    end_at: now + ChronoDuration::hours(2),
                }],
            },
        };

        let (programmes, statuses) =
            resolve_epg_programmes(&[primary_feed, fallback_feed], &lookup);

        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].title, "Primary Listing");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].last_program_count, Some(1));
    }

    fn sample_program_playback_row(
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
    ) -> ProgramPlaybackRow {
        ProgramPlaybackRow {
            id: Uuid::from_u128(7),
            title: "Matchday Live".to_string(),
            start_at,
            end_at,
            can_catchup: true,
            profile_id: Uuid::from_u128(9),
            channel_id: Some(Uuid::from_u128(8)),
            remote_stream_id: 42,
            stream_extension: Some("m3u8".to_string()),
            channel_name: "Arena 1".to_string(),
            has_catchup: true,
            base_url: "https://provider.example.com".to_string(),
            provider_username: "demo".to_string(),
            password_encrypted: "encrypted".to_string(),
            output_format: "m3u8".to_string(),
            playback_mode: "direct".to_string(),
        }
    }

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
            }),
            http_client: reqwest::Client::new(),
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
}
