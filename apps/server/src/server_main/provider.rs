use super::*;

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/provider", get(get_provider))
        .route("/provider/validate", post(validate_provider))
        .route("/provider/xtreme", put(save_provider))
        .route("/provider/sync", post(trigger_sync))
        .route("/provider/sync-status", get(get_sync_status))
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
    browser_playback_warning: Option<String>,
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
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
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
        browser_playback_warning: None,
        epg_sources: load_epg_sources(pool, provider.id).await?,
    }))
}

fn combine_provider_validation_message(message: &str, warning: Option<&str>) -> String {
    match warning {
        Some(warning) => format!("{message}\n\n{warning}"),
        None => message.to_string(),
    }
}

async fn browser_hls_warning_for_credentials(
    state: &AppState,
    credentials: &XtreamCredentials,
) -> Option<String> {
    match xtreme::probe_browser_hls_support(&state.provider_http_client, credentials).await {
        Ok(probe) => probe.warning_message,
        Err(error) => {
            warn!(error = ?error, "browser HLS support probe failed");
            Some(xtreme::browser_hls_warning_message().to_string())
        }
    }
}

fn normalize_epg_source_payloads(
    payloads: Vec<SaveEpgSourcePayload>,
) -> Result<Vec<SaveEpgSourcePayload>, AppError> {
    let mut deduped = Vec::new();
    let mut seen_urls = HashSet::new();

    for (index, payload) in payloads.into_iter().enumerate() {
        let url = payload.url.trim();
        if url.is_empty() {
            continue;
        }
        let url = Url::parse(url).map_err(|_| {
            AppError::BadRequest("EPG source URLs must be valid absolute URLs.".to_string())
        })?;
        let key = url.as_str().to_ascii_lowercase();
        if seen_urls.insert(key) {
            deduped.push(SaveEpgSourcePayload {
                id: payload.id,
                url: url.to_string(),
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
    let output_format = normalize_output_format(&payload.output_format)?;
    let existing_profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
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
        output_format: output_format_as_str(output_format).to_string(),
    };
    let result = xtreme::validate_profile(&state.provider_http_client, &credentials).await?;
    let browser_warning = if result.valid {
        browser_hls_warning_for_credentials(&state, &credentials).await
    } else {
        None
    };

    Ok(Json(ValidateProviderResponse {
        valid: result.valid,
        status: if result.valid { "valid" } else { "error" }.to_string(),
        message: combine_provider_validation_message(&result.message, browser_warning.as_deref()),
    }))
}

async fn save_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveProviderPayload>,
) -> ApiResult<ProviderProfileResponse> {
    let auth = require_auth(&state, &headers).await?;
    let output_format = normalize_output_format(&payload.output_format)?;
    let epg_sources = normalize_epg_source_payloads(payload.epg_sources)?;
    let existing_profile = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
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
        output_format: output_format_as_str(output_format).to_string(),
    };

    let validation = xtreme::validate_profile(&state.provider_http_client, &credentials).await?;
    if !validation.valid {
        return Err(AppError::BadRequest(validation.message));
    }
    let browser_playback_warning = browser_hls_warning_for_credentials(&state, &credentials).await;

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
    .bind(output_format_as_str(output_format))
    .bind(playback_mode_as_str(normalize_playback_mode(&payload.playback_mode)?))
    .fetch_one(&state.pool)
    .await?;

    store_epg_sources(&state.pool, profile_id, &epg_sources).await?;
    let provider = load_provider_profile_response(&state.pool, auth.user_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Provider profile was not found after saving.".to_string())
        })?;

    Ok(Json(ProviderProfileResponse {
        browser_playback_warning: browser_playback_warning,
        ..provider
    }))
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
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Connect a provider before starting sync".to_string()))?;

    sync::ensure_no_active_sync(&state.pool, profile.id).await?;
    let job =
        sync::insert_sync_job(&state.pool, auth.user_id, profile.id, "full", "manual").await?;

    sync::spawn_sync_job(state.clone(), auth.user_id, profile.id, job.id);
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
