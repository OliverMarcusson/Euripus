mod config;
mod xtreme;

use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

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
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{delete, get, post, put},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use config::Config;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use tokio::task::JoinHandle;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};
use uuid::Uuid;
use xtreme::{XtreamCategory, XtreamChannel, XtreamCredentials, XtreamProgramme};

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
    refresh_token: String,
    expires_at: DateTime<Utc>,
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

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct ProviderProfileResponse {
    id: Uuid,
    provider_type: String,
    base_url: String,
    username: String,
    output_format: String,
    status: String,
    last_validated_at: Option<DateTime<Utc>>,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, FromRow, Clone)]
#[serde(rename_all = "camelCase")]
struct ChannelResponse {
    id: Uuid,
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
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
    error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GuideResponse {
    categories: Vec<GuideCategorySummaryResponse>,
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
struct SearchResponse {
    query: String,
    channels: Vec<ChannelResponse>,
    programs: Vec<ProgramResponse>,
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ValidateProviderResponse {
    valid: bool,
    status: String,
    message: String,
}

#[derive(Debug, Deserialize)]
struct SearchParams {
    q: String,
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
    status: String,
    last_validated_at: Option<DateTime<Utc>>,
    last_sync_at: Option<DateTime<Utc>>,
    last_sync_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
struct ChannelPlaybackRecord {
    id: Uuid,
    name: String,
    remote_stream_id: i32,
    stream_extension: Option<String>,
    has_catchup: bool,
    archive_duration_hours: Option<i32>,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
}

const SYNC_BATCH_SIZE: usize = 10_000;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let config = Arc::new(Config::from_env()?);
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await
        .context("failed to connect to PostgreSQL")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

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
    let app = Router::new()
        .route("/health", get(health))
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/logout", post(logout))
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
        .route("/guide/category/{category_id}", get(get_guide_category))
        .route("/guide/channel/{id}", get(get_channel_guide))
        .route("/search", get(search_catalog))
        .route("/favorites", get(list_favorites))
        .route(
            "/favorites/{channel_id}",
            post(add_favorite).delete(remove_favorite),
        )
        .route("/recents", get(list_recents))
        .route("/playback/channel/{id}", post(play_channel))
        .route("/playback/program/{id}", post(play_program))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    info!("Euripus server listening on {bind_address}");
    let listener = tokio::net::TcpListener::bind(bind_address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> StatusCode {
    StatusCode::NO_CONTENT
}

async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CredentialsPayload>,
) -> ApiResult<AuthSessionResponse> {
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
    Ok(Json(session))
}

async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CredentialsPayload>,
) -> ApiResult<AuthSessionResponse> {
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
    Ok(Json(session))
}

async fn refresh_session(
    State(state): State<AppState>,
    Json(payload): Json<RefreshPayload>,
) -> ApiResult<AuthSessionResponse> {
    let refresh_hash = hash_refresh_token(&payload.refresh_token);
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

    let (access_token, expires_at) = create_access_token(&state, &user, session.id)?;
    Ok(Json(AuthSessionResponse {
        user: UserResponse {
            id: user.id,
            username: user.username,
            created_at: user.created_at,
        },
        access_token,
        refresh_token: next_refresh_token,
        expires_at,
    }))
}

async fn logout(
    State(state): State<AppState>,
    Json(payload): Json<RefreshPayload>,
) -> Result<StatusCode, AppError> {
    let refresh_hash = hash_refresh_token(&payload.refresh_token);
    sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE refresh_token_hash = $1")
        .bind(refresh_hash)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
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

async fn get_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Option<ProviderProfileResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let provider = sqlx::query_as::<_, ProviderProfileResponse>(
        r#"
        SELECT
          id,
          provider_type,
          base_url,
          username,
          output_format,
          status,
          last_validated_at,
          last_sync_at,
          last_sync_error,
          created_at,
          updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(provider))
}

async fn validate_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveProviderPayload>,
) -> ApiResult<ValidateProviderResponse> {
    let _auth = require_auth(&state, &headers).await?;
    let credentials = XtreamCredentials {
        base_url: payload.base_url,
        username: payload.username,
        password: payload.password,
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
    let credentials = XtreamCredentials {
        base_url: payload.base_url.clone(),
        username: payload.username.clone(),
        password: payload.password.clone(),
        output_format: payload.output_format.clone(),
    };

    let validation = xtreme::validate_profile(&state.http_client, &credentials).await?;
    if !validation.valid {
        return Err(AppError::BadRequest(validation.message));
    }

    let encrypted_password = encrypt_secret(&state.config.encryption_key, &payload.password)?;
    let provider = sqlx::query_as::<_, ProviderProfileResponse>(
        r#"
        INSERT INTO provider_profiles (
          user_id, provider_type, base_url, username, password_encrypted, output_format, status, last_validated_at, last_sync_error
        )
        VALUES ($1, 'xtreme', $2, $3, $4, $5, 'valid', NOW(), NULL)
        ON CONFLICT (user_id)
        DO UPDATE SET
          provider_type = 'xtreme',
          base_url = EXCLUDED.base_url,
          username = EXCLUDED.username,
          password_encrypted = EXCLUDED.password_encrypted,
          output_format = EXCLUDED.output_format,
          status = 'valid',
          last_validated_at = NOW(),
          last_sync_error = NULL,
          updated_at = NOW()
        RETURNING
          id,
          provider_type,
          base_url,
          username,
          output_format,
          status,
          last_validated_at,
          last_sync_at,
          last_sync_error,
          created_at,
          updated_at
        "#,
    )
    .bind(auth.user_id)
    .bind(payload.base_url)
    .bind(payload.username)
    .bind(encrypted_password)
    .bind(payload.output_format)
    .fetch_one(&state.pool)
    .await?;

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
          id, user_id, provider_type, base_url, username, password_encrypted, output_format,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Connect a provider before starting sync".to_string()))?;

    let job = sqlx::query_as::<_, SyncJobResponse>(
        r#"
        INSERT INTO sync_jobs (user_id, profile_id, status, job_type)
        VALUES ($1, $2, 'queued', 'full')
        RETURNING id, status, job_type, created_at, started_at, finished_at, error_message
        "#,
    )
    .bind(auth.user_id)
    .bind(profile.id)
    .fetch_one(&state.pool)
    .await?;

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
        SELECT id, status, job_type, created_at, started_at, finished_at, error_message
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
        let mut interval = tokio::time::interval(Duration::from_secs(60 * 30));
        loop {
            interval.tick().await;
            if let Err(error) = queue_stale_syncs(state.clone()).await {
                error!("periodic sync worker failed: {error:?}");
            }
        }
    })
}

async fn queue_stale_syncs(state: AppState) -> Result<()> {
    let stale_profiles = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE status = 'valid'
          AND (last_sync_at IS NULL OR last_sync_at < NOW() - INTERVAL '6 hours')
        "#,
    )
    .fetch_all(&state.pool)
    .await?;

    for profile in stale_profiles {
        let active_job = sqlx::query_scalar::<_, i64>(
            r#"SELECT COUNT(*) FROM sync_jobs WHERE profile_id = $1 AND status IN ('queued', 'running')"#,
        )
        .bind(profile.id)
        .fetch_one(&state.pool)
        .await?;

        if active_job > 0 {
            continue;
        }

        let job = sqlx::query_as::<_, SyncJobResponse>(
            r#"
            INSERT INTO sync_jobs (user_id, profile_id, status, job_type)
            VALUES ($1, $2, 'queued', 'epg')
            RETURNING id, status, job_type, created_at, started_at, finished_at, error_message
            "#,
        )
        .bind(profile.user_id)
        .bind(profile.id)
        .fetch_one(&state.pool)
        .await?;

        spawn_sync_job(state.clone(), profile.user_id, profile.id, job.id);
    }

    Ok(())
}

async fn list_channels(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<ChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let channels = fetch_channels(&state.pool, auth.user_id).await?;
    Ok(Json(channels))
}

async fn get_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<ChannelResponse> {
    let auth = require_auth(&state, &headers).await?;
    let channel = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
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

    Ok(Json(channel))
}

async fn get_guide(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<GuideResponse> {
    let auth = require_auth(&state, &headers).await?;
    let categories = fetch_guide_categories(&state.pool, auth.user_id).await?;
    Ok(Json(GuideResponse { categories }))
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
    let entries = rows
        .into_iter()
        .map(map_guide_category_entry)
        .collect::<Vec<_>>();

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

async fn search_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<SearchParams>,
) -> ApiResult<SearchResponse> {
    let auth = require_auth(&state, &headers).await?;
    let channels = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
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
        FROM search_documents sd
        JOIN channels c ON c.id = sd.entity_id
        LEFT JOIN channel_categories cc ON cc.id = c.category_id
        WHERE sd.user_id = $1
          AND sd.entity_type = 'channel'
          AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
        ORDER BY
          CASE WHEN lower(sd.title) = lower($2) THEN 0 ELSE 1 END,
          similarity(sd.search_text, $2) DESC,
          sd.title ASC
        LIMIT 25
        "#,
    )
    .bind(auth.user_id)
    .bind(&params.q)
    .fetch_all(&state.pool)
    .await?;

    let programs = sqlx::query_as::<_, ProgramResponse>(
        r#"
        SELECT
          p.id,
          p.channel_id,
          p.channel_name,
          p.title,
          p.description,
          p.start_at,
          p.end_at,
          p.can_catchup
        FROM search_documents sd
        JOIN programs p ON p.id = sd.entity_id
        WHERE sd.user_id = $1
          AND sd.entity_type = 'program'
          AND (sd.tsv @@ plainto_tsquery('simple', $2) OR sd.search_text % $2)
        ORDER BY
          CASE
            WHEN lower(sd.title) = lower($2) THEN 0
            WHEN p.start_at <= NOW() AND p.end_at >= NOW() THEN 1
            ELSE 2
          END,
          similarity(sd.search_text, $2) DESC,
          p.start_at ASC
        LIMIT 25
        "#,
    )
    .bind(auth.user_id)
    .bind(&params.q)
    .fetch_all(&state.pool)
    .await?;

    Ok(Json(SearchResponse {
        query: params.q,
        channels,
        programs,
    }))
}

async fn list_favorites(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<ChannelResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let favorites = sqlx::query_as::<_, ChannelResponse>(
        r#"
        SELECT
          c.id,
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

    let recents = rows
        .into_iter()
        .map(|row| RecentChannelResponse {
            channel: ChannelResponse {
                id: row.id,
                name: row.name,
                logo_url: row.logo_url,
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
        .collect();

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
          c.name,
          c.remote_stream_id,
          c.stream_extension,
          c.has_catchup,
          c.archive_duration_hours,
          p.base_url,
          p.username AS provider_username,
          p.password_encrypted,
          p.output_format
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

    Ok(Json(playback_source_from_url(
        &record.name,
        url,
        true,
        false,
        record.stream_extension.as_deref(),
        None,
    )))
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
          c.id AS channel_id,
          c.remote_stream_id,
          c.stream_extension,
          c.name AS channel_name,
          c.has_catchup,
          pr.base_url,
          pr.username AS provider_username,
          pr.password_encrypted,
          pr.output_format
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

    let Some(channel_id) = row.channel_id else {
        return Ok(Json(unsupported_playback(
            &row.title,
            "This program is not mapped to a playable channel.",
        )));
    };
    touch_recent(&state.pool, auth.user_id, channel_id).await?;

    if !row.can_catchup || !row.has_catchup {
        return Ok(Json(unsupported_playback(
            &row.title,
            "Catch-up is not available for this program on the provider.",
        )));
    }

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

    Ok(Json(playback_source_from_url(
        &row.title,
        url,
        false,
        true,
        row.stream_extension.as_deref(),
        None,
    )))
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

fn map_guide_category_entry(row: GuideCategoryEntryRow) -> GuideChannelEntryResponse {
    GuideChannelEntryResponse {
        channel: ChannelResponse {
            id: row.channel_id,
            name: row.channel_name,
            logo_url: row.logo_url,
            category_name: row.category_name,
            remote_stream_id: row.remote_stream_id,
            epg_channel_id: row.epg_channel_id,
            has_catchup: row.has_catchup,
            archive_duration_hours: row.archive_duration_hours,
            stream_extension: row.stream_extension,
            is_favorite: row.is_favorite,
        },
        program: row.program_id.map(|id| ProgramResponse {
            id,
            channel_id: row.program_channel_id,
            channel_name: row.program_channel_name,
            title: row.program_title.unwrap_or_default(),
            description: row.program_description,
            start_at: row
                .program_start_at
                .expect("program_start_at should exist when program_id exists"),
            end_at: row
                .program_end_at
                .expect("program_end_at should exist when program_id exists"),
            can_catchup: row.program_can_catchup.unwrap_or(false),
        }),
    }
}

fn next_guide_offset(offset: i64, limit: i64, total_count: i64) -> Option<i64> {
    let next_offset = offset + limit;
    (next_offset < total_count).then_some(next_offset)
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
) -> Result<AuthSessionResponse, AppError> {
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

    let (access_token, access_expires_at) = create_access_token(state, user, session.id)?;
    Ok(AuthSessionResponse {
        user: UserResponse {
            id: user.id,
            username: user.username.clone(),
            created_at: user.created_at,
        },
        access_token,
        refresh_token,
        expires_at: access_expires_at,
    })
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
                SET status = 'failed', finished_at = NOW(), error_message = $2
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
    sqlx::query("UPDATE sync_jobs SET status = 'running', started_at = NOW() WHERE id = $1")
        .bind(job_id)
        .execute(&state.pool)
        .await?;
    sqlx::query(
        r#"UPDATE provider_profiles SET status = 'syncing', last_sync_error = NULL, updated_at = NOW() WHERE id = $1"#,
    )
    .bind(profile_id)
    .execute(&state.pool)
    .await?;

    let profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
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

    info!("sync job {job_id}: validating provider");
    let validation = xtreme::validate_profile(&state.http_client, &credentials).await?;
    if !validation.valid {
        return Err(anyhow!("provider validation failed during sync"));
    }

    info!("sync job {job_id}: fetching categories");
    let categories = xtreme::fetch_categories(&state.http_client, &credentials).await?;
    info!("sync job {job_id}: fetched {} categories", categories.len());
    info!("sync job {job_id}: fetching live streams");
    let channels = xtreme::fetch_live_streams(&state.http_client, &credentials).await?;
    info!("sync job {job_id}: fetched {} live streams", channels.len());
    info!("sync job {job_id}: fetching xmltv");
    let programmes = xtreme::fetch_xmltv(&state.http_client, &credentials).await?;
    info!(
        "sync job {job_id}: parsed {} xmltv programmes",
        programmes.len()
    );

    info!("sync job {job_id}: persisting sync data");
    persist_sync_data(
        &state.pool,
        user_id,
        profile_id,
        &categories,
        &channels,
        &programmes,
    )
    .await?;
    info!("sync job {job_id}: finished persisting sync data");

    sqlx::query(
        r#"
        UPDATE sync_jobs
        SET status = 'succeeded', finished_at = NOW(), error_message = NULL
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

async fn persist_sync_data(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
    categories: &[XtreamCategory],
    channels: &[XtreamChannel],
    programmes: &[XtreamProgramme],
) -> Result<()> {
    let mut transaction = pool.begin().await?;
    bulk_upsert_categories(&mut transaction, user_id, profile_id, categories).await?;
    bulk_upsert_channels(&mut transaction, user_id, profile_id, channels).await?;

    sqlx::query("DELETE FROM programs WHERE user_id = $1 AND profile_id = $2")
        .bind(user_id)
        .bind(profile_id)
        .execute(&mut *transaction)
        .await?;

    prepare_channel_lookup(&mut transaction, user_id, profile_id).await?;
    bulk_insert_programmes(&mut transaction, user_id, profile_id, programmes).await?;

    sqlx::query("DELETE FROM search_documents WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *transaction)
        .await?;

    rebuild_search_documents(&mut transaction, user_id).await?;

    transaction.commit().await?;
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

async fn prepare_channel_lookup(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TEMP TABLE sync_channel_lookup (
          channel_key TEXT PRIMARY KEY,
          channel_id UUID NOT NULL,
          channel_name TEXT NOT NULL,
          has_catchup BOOLEAN NOT NULL
        ) ON COMMIT DROP
        "#,
    )
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO sync_channel_lookup (channel_key, channel_id, channel_name, has_catchup)
        SELECT c.remote_stream_id::text, c.id, c.name, c.has_catchup
        FROM channels c
        WHERE c.user_id = $1 AND c.profile_id = $2
        ON CONFLICT (channel_key)
        DO UPDATE SET
          channel_id = EXCLUDED.channel_id,
          channel_name = EXCLUDED.channel_name,
          has_catchup = EXCLUDED.has_catchup
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO sync_channel_lookup (channel_key, channel_id, channel_name, has_catchup)
        SELECT DISTINCT ON (c.epg_channel_id) c.epg_channel_id, c.id, c.name, c.has_catchup
        FROM channels c
        WHERE c.user_id = $1
          AND c.profile_id = $2
          AND c.epg_channel_id IS NOT NULL
        ORDER BY c.epg_channel_id, c.updated_at DESC, c.id DESC
        ON CONFLICT (channel_key)
        DO UPDATE SET
          channel_id = EXCLUDED.channel_id,
          channel_name = EXCLUDED.channel_name,
          has_catchup = EXCLUDED.has_catchup
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .execute(&mut **transaction)
    .await?;

    Ok(())
}

async fn bulk_insert_programmes(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    profile_id: Uuid,
    programmes: &[XtreamProgramme],
) -> Result<()> {
    for chunk in programmes.chunks(SYNC_BATCH_SIZE) {
        let channel_keys = chunk
            .iter()
            .map(|programme| programme.channel_key.clone())
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

        sqlx::query(
            r#"
            WITH input AS (
              SELECT *
              FROM UNNEST(
                $3::text[],
                $4::text[],
                $5::text[],
                $6::timestamptz[],
                $7::timestamptz[]
              ) AS input(channel_key, title, description, start_at, end_at)
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
              lookup.channel_id,
              lookup.channel_name,
              input.title,
              input.description,
              input.start_at,
              input.end_at,
              COALESCE(lookup.has_catchup, FALSE)
            FROM input
            LEFT JOIN sync_channel_lookup lookup ON lookup.channel_key = input.channel_key
            "#,
        )
        .bind(user_id)
        .bind(profile_id)
        .bind(&channel_keys)
        .bind(&titles)
        .bind(&descriptions)
        .bind(&start_times)
        .bind(&end_times)
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
    channel_id: Option<Uuid>,
    remote_stream_id: i32,
    stream_extension: Option<String>,
    channel_name: String,
    has_catchup: bool,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
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
    fn maps_guide_entry_rows_into_nested_payloads() {
        let now = Utc::now();
        let entry = map_guide_category_entry(GuideCategoryEntryRow {
            channel_id: Uuid::nil(),
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
        });

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

    #[test]
    fn maps_guide_entry_rows_without_programs() {
        let entry = map_guide_category_entry(GuideCategoryEntryRow {
            channel_id: Uuid::nil(),
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
        });

        assert_eq!(entry.channel.name, "Arena 2");
        assert!(entry.program.is_none());
    }
}
