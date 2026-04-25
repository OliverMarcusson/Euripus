use super::*;

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/providers", get(list_providers))
        .route("/providers/validate", post(validate_provider))
        .route("/providers/xtreme", post(save_provider))
        .route("/providers/{id}", delete(delete_provider))
        .route("/providers/{id}/sync", post(trigger_sync))
        .route("/providers/{id}/sync-status", get(get_sync_status))
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
    id: Option<Uuid>,
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

fn provider_profile_response_from_record(
    provider: ProviderProfileRecord,
    epg_sources: Vec<EpgSourceResponse>,
) -> ProviderProfileResponse {
    ProviderProfileResponse {
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
        epg_sources,
    }
}

async fn load_provider_profile_record(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<Option<ProviderProfileRecord>, AppError> {
    sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1 AND id = $2
        "#,
    )
    .bind(user_id)
    .bind(profile_id)
    .fetch_optional(pool)
    .await
    .map_err(Into::into)
}

async fn load_provider_profile_response(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<Option<ProviderProfileResponse>, AppError> {
    let provider = load_provider_profile_record(pool, user_id, profile_id).await?;
    let Some(provider) = provider else {
        return Ok(None);
    };

    let epg_sources = load_epg_sources(pool, provider.id).await?;
    Ok(Some(provider_profile_response_from_record(
        provider,
        epg_sources,
    )))
}

async fn load_provider_profile_responses(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<ProviderProfileResponse>, AppError> {
    let providers = sqlx::query_as::<_, ProviderProfileRecord>(
        r#"
        SELECT
          id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode,
          status, last_validated_at, last_sync_at, last_sync_error, created_at, updated_at
        FROM provider_profiles
        WHERE user_id = $1
        ORDER BY updated_at DESC, created_at DESC, id ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let mut responses = Vec::with_capacity(providers.len());
    for provider in providers {
        let epg_sources = load_epg_sources(pool, provider.id).await?;
        responses.push(provider_profile_response_from_record(provider, epg_sources));
    }

    Ok(responses)
}

fn stored_password_reentry_message() -> String {
    "Stored provider password could not be decrypted. Re-enter your provider password and save the profile again.".to_string()
}

fn resolve_effective_password(
    encryption_key: &[u8; 32],
    existing_profile: Option<&ProviderProfileRecord>,
    submitted_password: &str,
    missing_password_message: &str,
) -> Result<String, AppError> {
    if !submitted_password.trim().is_empty() {
        return Ok(submitted_password.to_string());
    }

    let Some(profile) = existing_profile else {
        return Err(AppError::BadRequest(missing_password_message.to_string()));
    };

    decrypt_secret(encryption_key, &profile.password_encrypted)
        .map_err(|_| AppError::BadRequest(stored_password_reentry_message()))
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

async fn require_target_provider_profile(
    pool: &PgPool,
    user_id: Uuid,
    profile_id: Uuid,
) -> Result<ProviderProfileRecord, AppError> {
    load_provider_profile_record(pool, user_id, profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Provider profile not found.".to_string()))
}

async fn list_providers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let providers = load_provider_profile_responses(&state.pool, auth.user_id).await?;

    json_response_with_revalidation(&headers, &providers)
}

async fn validate_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SaveProviderPayload>,
) -> ApiResult<ValidateProviderResponse> {
    let auth = require_auth(&state, &headers).await?;
    let output_format = normalize_output_format(&payload.output_format)?;
    let existing_profile = match payload.id {
        Some(profile_id) => {
            Some(require_target_provider_profile(&state.pool, auth.user_id, profile_id).await?)
        }
        None => None,
    };
    let effective_password = resolve_effective_password(
        &state.config.encryption_key,
        existing_profile.as_ref(),
        &payload.password,
        "Enter your provider password when validating the profile for the first time.",
    )?;
    let credentials = XtreamCredentials {
        base_url: payload.base_url,
        username: payload.username,
        password: effective_password,
        output_format: output_format_as_str(output_format).to_string(),
    };
    let result = xtreme::validate_profile(&state.provider_http_client, &credentials).await?;

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
    let output_format = normalize_output_format(&payload.output_format)?;
    let playback_mode = normalize_playback_mode(&payload.playback_mode)?;
    let epg_sources = normalize_epg_source_payloads(payload.epg_sources)?;
    let existing_profile = match payload.id {
        Some(profile_id) => {
            Some(require_target_provider_profile(&state.pool, auth.user_id, profile_id).await?)
        }
        None => None,
    };
    let effective_password = resolve_effective_password(
        &state.config.encryption_key,
        existing_profile.as_ref(),
        &payload.password,
        "Enter your provider password when saving the profile for the first time.",
    )?;
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
    let encrypted_password = encrypt_secret(&state.config.encryption_key, &effective_password)?;
    let profile_id = if let Some(existing_profile) = existing_profile.as_ref() {
        sqlx::query(
            r#"
            UPDATE provider_profiles
            SET
              provider_type = 'xtreme',
              base_url = $3,
              username = $4,
              password_encrypted = $5,
              output_format = $6,
              playback_mode = $7,
              status = 'valid',
              last_validated_at = NOW(),
              last_sync_error = NULL,
              updated_at = NOW()
            WHERE user_id = $1 AND id = $2
            "#,
        )
        .bind(auth.user_id)
        .bind(existing_profile.id)
        .bind(&payload.base_url)
        .bind(&payload.username)
        .bind(encrypted_password)
        .bind(output_format_as_str(output_format))
        .bind(playback_mode_as_str(playback_mode))
        .execute(&state.pool)
        .await?;
        existing_profile.id
    } else {
        let profile_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO provider_profiles (
              id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode, status, last_validated_at, last_sync_error
            )
            VALUES ($1, $2, 'xtreme', $3, $4, $5, $6, $7, 'valid', NOW(), NULL)
            "#,
        )
        .bind(profile_id)
        .bind(auth.user_id)
        .bind(&payload.base_url)
        .bind(&payload.username)
        .bind(encrypted_password)
        .bind(output_format_as_str(output_format))
        .bind(playback_mode_as_str(playback_mode))
        .execute(&state.pool)
        .await?;
        profile_id
    };

    store_epg_sources(&state.pool, profile_id, &epg_sources).await?;
    let provider = load_provider_profile_response(&state.pool, auth.user_id, profile_id)
        .await?
        .ok_or_else(|| {
            AppError::NotFound("Provider profile was not found after saving.".to_string())
        })?;

    Ok(Json(ProviderProfileResponse {
        browser_playback_warning: None,
        ..provider
    }))
}

async fn delete_provider(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let profile = require_target_provider_profile(&state.pool, auth.user_id, id).await?;

    sync::ensure_no_active_sync(&state.pool, profile.id).await?;

    sqlx::query("DELETE FROM provider_profiles WHERE user_id = $1 AND id = $2")
        .bind(auth.user_id)
        .bind(profile.id)
        .execute(&state.pool)
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn trigger_sync(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<SyncJobResponse> {
    let auth = require_auth(&state, &headers).await?;
    let profile = require_target_provider_profile(&state.pool, auth.user_id, id).await?;

    decrypt_secret(&state.config.encryption_key, &profile.password_encrypted)
        .map_err(|_| AppError::BadRequest(stored_password_reentry_message()))?;

    let job =
        sync::insert_sync_job(&state.pool, auth.user_id, profile.id, "full", "manual").await?;

    sync::spawn_sync_job(state.clone(), auth.user_id, profile.id, job.id);
    Ok(Json(job))
}

async fn get_sync_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<Option<SyncJobResponse>> {
    let auth = require_auth(&state, &headers).await?;
    require_target_provider_profile(&state.pool, auth.user_id, id).await?;

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
        WHERE user_id = $1 AND profile_id = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;

    Ok(Json(job))
}
