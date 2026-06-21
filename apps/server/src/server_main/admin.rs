use super::search::{lexicon, rules};
use super::*;

const ADMIN_SESSION_COOKIE_NAME: &str = "euripus.admin";
const ADMIN_CSRF_COOKIE_NAME: &str = "euripus.admin.csrf";

pub(super) fn browser_router() -> Router<AppState> {
    Router::new()
        .route("/admin/auth/login", post(login))
        .route("/admin/auth/logout", post(logout))
        .route(
            "/admin/restricted-accounts",
            get(list_restricted_accounts).post(create_restricted_account),
        )
        .route(
            "/admin/restricted-accounts/{id}",
            put(update_restricted_account).delete(delete_restricted_account),
        )
        .route(
            "/admin/search/pattern-groups",
            get(list_pattern_groups)
                .post(create_pattern_group)
                .delete(delete_all_pattern_groups),
        )
        .route(
            "/admin/search/pattern-group-import",
            post(import_pattern_groups),
        )
        .route(
            "/admin/search/pattern-groups/{id}",
            put(update_pattern_group).delete(delete_pattern_group),
        )
        .route("/admin/search/test", post(test_patterns))
        .route("/admin/search/test-query", post(test_search_query))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminPatternResponse {
    id: Uuid,
    pattern: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminPatternGroupResponse {
    id: Uuid,
    kind: AdminSearchPatternKind,
    value: String,
    normalized_value: String,
    match_target: AdminSearchMatchTarget,
    match_mode: AdminSearchMatchMode,
    priority: i32,
    enabled: bool,
    patterns_text: String,
    country_codes_text: String,
    country_codes: Vec<String>,
    patterns: Vec<AdminPatternResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminLoginPayload {
    password: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedEpgSourcePayload {
    url: String,
    enabled: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedProviderPayload {
    base_url: String,
    username: String,
    password: String,
    output_format: String,
    playback_mode: String,
    #[serde(default)]
    epg_sources: Vec<ManagedEpgSourcePayload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedAccountPayload {
    username: String,
    #[serde(default)]
    password: String,
    provider: ManagedProviderPayload,
}

#[derive(Debug, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
struct ManagedAccountResponse {
    id: Uuid,
    username: String,
    created_at: DateTime<Utc>,
    provider_id: Option<Uuid>,
    provider_status: Option<String>,
    provider_last_sync_at: Option<DateTime<Utc>>,
    provider_last_sync_error: Option<String>,
    provider_base_url: Option<String>,
    provider_username: Option<String>,
    provider_output_format: Option<String>,
    provider_playback_mode: Option<String>,
    provider_epg_urls: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminPatternGroupPayload {
    kind: AdminSearchPatternKind,
    value: String,
    match_target: AdminSearchMatchTarget,
    match_mode: AdminSearchMatchMode,
    priority: i32,
    enabled: bool,
    patterns_text: String,
    country_codes_text: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSearchTestPayload {
    channel_name: Option<String>,
    category_name: Option<String>,
    program_title: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminSearchQueryTestPayload {
    query: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdminPatternGroupImportPayload {
    groups: Vec<AdminPatternGroupImportItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AdminPatternGroupImportItem {
    kind: String,
    value: String,
    match_target: String,
    match_mode: String,
    priority: Option<i32>,
    enabled: Option<bool>,
    patterns: Option<Vec<String>>,
    patterns_text: Option<String>,
    country_codes: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSearchTestResponse {
    country_code: Option<String>,
    provider_name: Option<String>,
    is_ppv: bool,
    is_vip: bool,
    force_has_epg: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminSearchQueryTestResponse {
    search: String,
    countries: Vec<String>,
    providers: Vec<String>,
    ppv: Option<bool>,
    vip: Option<bool>,
    require_epg: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdminPatternGroupImportErrorDetail {
    index: usize,
    field: String,
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct AdminAccessClaims {
    role: String,
    exp: usize,
}

#[derive(Debug, Clone)]
struct ValidatedPatternGroupInput {
    kind: AdminSearchPatternKind,
    value: String,
    match_target: AdminSearchMatchTarget,
    match_mode: AdminSearchMatchMode,
    priority: i32,
    enabled: bool,
    patterns: Vec<String>,
    country_codes: Vec<String>,
}

async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(payload): Json<AdminLoginPayload>,
) -> Result<(CookieJar, StatusCode), AppError> {
    let Some(admin_password) = state.config.admin_password.as_deref() else {
        return Err(AppError::BadRequest(
            "Admin authentication is not configured on this server".to_string(),
        ));
    };

    if payload.password != admin_password {
        return Err(AppError::BadRequest("Invalid admin password".to_string()));
    }

    let expires_at = Utc::now() + ChronoDuration::days(state.config.refresh_token_days);
    let claims = AdminAccessClaims {
        role: "admin".to_string(),
        exp: expires_at.timestamp() as usize,
    };
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.config.jwt_secret.as_bytes()),
    )
    .map_err(|error| AppError::Internal(anyhow!(error)))?;
    let csrf_token = generate_refresh_token();

    Ok((
        set_admin_auth_cookies(&state, jar, token, csrf_token),
        StatusCode::NO_CONTENT,
    ))
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<(CookieJar, StatusCode), AppError> {
    validate_admin_csrf(&jar, &headers)?;
    Ok((
        clear_admin_auth_cookies(&state, jar),
        StatusCode::NO_CONTENT,
    ))
}

async fn list_restricted_accounts(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Vec<ManagedAccountResponse>> {
    require_admin(&state, &jar)?;
    let accounts = sqlx::query_as::<_, ManagedAccountResponse>(
        r#"
        SELECT u.id, u.username, u.created_at,
          p.id AS provider_id, p.status AS provider_status,
          p.last_sync_at AS provider_last_sync_at, p.last_sync_error AS provider_last_sync_error,
          p.base_url AS provider_base_url, p.username AS provider_username,
          p.output_format AS provider_output_format, p.playback_mode AS provider_playback_mode,
          COALESCE((SELECT array_agg(e.url ORDER BY e.priority, e.created_at) FROM epg_sources e WHERE e.profile_id = p.id), ARRAY[]::TEXT[]) AS provider_epg_urls
        FROM users u
        LEFT JOIN LATERAL (
          SELECT id, status, last_sync_at, last_sync_error, base_url, username, output_format, playback_mode FROM provider_profiles
          WHERE user_id = u.id ORDER BY created_at ASC LIMIT 1
        ) p ON TRUE
        WHERE u.provider_locked = TRUE
        ORDER BY u.created_at DESC
        "#,
    ).fetch_all(&state.pool).await?;
    Ok(Json(accounts))
}

async fn create_restricted_account(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<ManagedAccountPayload>,
) -> ApiResult<ManagedAccountResponse> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let username = normalize_managed_username(&payload.username)?;
    if payload.password.is_empty() {
        return Err(AppError::BadRequest(
            "A login password is required.".to_string(),
        ));
    }
    let password_hash = hash_password(&payload.password)?;
    let user_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO users (username, password_hash, provider_locked) VALUES ($1, $2, TRUE) RETURNING id",
    ).bind(&username).bind(password_hash).fetch_one(&state.pool).await.map_err(map_managed_user_error)?;
    if let Err(error) = save_managed_provider(&state, user_id, None, payload.provider).await {
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&state.pool)
            .await?;
        return Err(error);
    }
    load_managed_account(&state.pool, user_id).await
}

async fn update_restricted_account(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<ManagedAccountPayload>,
) -> ApiResult<ManagedAccountResponse> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let username = normalize_managed_username(&payload.username)?;
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND provider_locked = TRUE)",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?;
    if !exists {
        return Err(AppError::NotFound(
            "Restricted account not found.".to_string(),
        ));
    }
    if payload.password.trim().is_empty() {
        sqlx::query("UPDATE users SET username = $1 WHERE id = $2")
            .bind(&username)
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(map_managed_user_error)?;
    } else {
        sqlx::query("UPDATE users SET username = $1, password_hash = $2 WHERE id = $3")
            .bind(&username)
            .bind(hash_password(&payload.password)?)
            .bind(id)
            .execute(&state.pool)
            .await
            .map_err(map_managed_user_error)?;
    }
    let profile_id = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM provider_profiles WHERE user_id = $1 ORDER BY created_at ASC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await?;
    save_managed_provider(&state, id, profile_id, payload.provider).await?;
    load_managed_account(&state.pool, id).await
}

async fn delete_restricted_account(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let deleted = sqlx::query("DELETE FROM users WHERE id = $1 AND provider_locked = TRUE")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Restricted account not found.".to_string(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

fn normalize_managed_username(value: &str) -> Result<String, AppError> {
    let username = value.trim().to_ascii_lowercase();
    if username.len() < 3 {
        return Err(AppError::BadRequest(
            "Username must be at least 3 characters.".to_string(),
        ));
    }
    Ok(username)
}

fn map_managed_user_error(error: sqlx::Error) -> AppError {
    match error {
        sqlx::Error::Database(database_error) if database_error.is_unique_violation() => {
            AppError::BadRequest("That username is already taken.".to_string())
        }
        other => AppError::Internal(anyhow!(other)),
    }
}

async fn save_managed_provider(
    state: &AppState,
    user_id: Uuid,
    profile_id: Option<Uuid>,
    payload: ManagedProviderPayload,
) -> Result<(), AppError> {
    let output_format = normalize_output_format(&payload.output_format)?;
    let playback_mode = normalize_playback_mode(&payload.playback_mode)?;
    let password = if payload.password.trim().is_empty() {
        let id = profile_id
            .ok_or_else(|| AppError::BadRequest("A provider password is required.".to_string()))?;
        let encrypted = sqlx::query_scalar::<_, String>(
            "SELECT password_encrypted FROM provider_profiles WHERE id = $1 AND user_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_one(&state.pool)
        .await?;
        decrypt_secret(&state.config.encryption_key, &encrypted).map_err(|_| {
            AppError::BadRequest(
                "Stored provider password could not be decrypted. Enter it again.".to_string(),
            )
        })?
    } else {
        payload.password
    };
    let credentials = XtreamCredentials {
        base_url: payload.base_url.clone(),
        username: payload.username.clone(),
        password: password.clone(),
        output_format: output_format_as_str(output_format).to_string(),
    };
    let validation = xtreme::validate_profile(&state.provider_http_client, &credentials).await?;
    if !validation.valid {
        return Err(AppError::BadRequest(validation.message));
    }
    let id = profile_id.unwrap_or_else(Uuid::new_v4);
    if profile_id.is_some() {
        sqlx::query("UPDATE provider_profiles SET base_url = $2, username = $3, password_encrypted = $4, output_format = $5, playback_mode = $6, status = 'valid', last_validated_at = NOW(), last_sync_error = NULL, updated_at = NOW() WHERE id = $1 AND user_id = $7")
            .bind(id).bind(&payload.base_url).bind(&payload.username).bind(encrypt_secret(&state.config.encryption_key, &password)?).bind(output_format_as_str(output_format)).bind(playback_mode_as_str(playback_mode)).bind(user_id).execute(&state.pool).await?;
    } else {
        sqlx::query("INSERT INTO provider_profiles (id, user_id, provider_type, base_url, username, password_encrypted, output_format, playback_mode, status, last_validated_at) VALUES ($1, $2, 'xtreme', $3, $4, $5, $6, $7, 'valid', NOW())")
            .bind(id).bind(user_id).bind(&payload.base_url).bind(&payload.username).bind(encrypt_secret(&state.config.encryption_key, &password)?).bind(output_format_as_str(output_format)).bind(playback_mode_as_str(playback_mode)).execute(&state.pool).await?;
    }
    sqlx::query("DELETE FROM epg_sources WHERE profile_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    for (index, source) in payload.epg_sources.into_iter().enumerate() {
        let url = Url::parse(source.url.trim()).map_err(|_| {
            AppError::BadRequest("EPG source URLs must be valid absolute URLs.".to_string())
        })?;
        sqlx::query(
            "INSERT INTO epg_sources (profile_id, url, priority, enabled) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(url.to_string())
        .bind(index as i32)
        .bind(source.enabled)
        .execute(&state.pool)
        .await?;
    }
    let job = sync::insert_sync_job(&state.pool, user_id, id, "full", "manual").await?;
    sync::spawn_sync_job(state.clone(), user_id, id, job.id);
    Ok(())
}

async fn load_managed_account(pool: &PgPool, id: Uuid) -> ApiResult<ManagedAccountResponse> {
    let account = sqlx::query_as::<_, ManagedAccountResponse>(r#"SELECT u.id, u.username, u.created_at, p.id AS provider_id, p.status AS provider_status, p.last_sync_at AS provider_last_sync_at, p.last_sync_error AS provider_last_sync_error, p.base_url AS provider_base_url, p.username AS provider_username, p.output_format AS provider_output_format, p.playback_mode AS provider_playback_mode, COALESCE((SELECT array_agg(e.url ORDER BY e.priority, e.created_at) FROM epg_sources e WHERE e.profile_id = p.id), ARRAY[]::TEXT[]) AS provider_epg_urls FROM users u LEFT JOIN LATERAL (SELECT id, status, last_sync_at, last_sync_error, base_url, username, output_format, playback_mode FROM provider_profiles WHERE user_id = u.id ORDER BY created_at ASC LIMIT 1) p ON TRUE WHERE u.id = $1 AND u.provider_locked = TRUE"#).bind(id).fetch_optional(pool).await?.ok_or_else(|| AppError::NotFound("Restricted account not found.".to_string()))?;
    Ok(Json(account))
}

async fn list_pattern_groups(
    State(state): State<AppState>,
    jar: CookieJar,
) -> ApiResult<Vec<AdminPatternGroupResponse>> {
    require_admin(&state, &jar)?;
    let groups = rules::load_pattern_groups(&state.pool).await?;
    Ok(Json(
        groups.into_iter().map(map_pattern_group_response).collect(),
    ))
}

async fn create_pattern_group(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<AdminPatternGroupPayload>,
) -> ApiResult<AdminPatternGroupResponse> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let group = save_pattern_group(&state.pool, None, payload).await?;
    spawn_admin_reindex(state.clone());
    Ok(Json(group))
}

async fn import_pattern_groups(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<AdminPatternGroupImportPayload>,
) -> ApiResult<Vec<AdminPatternGroupResponse>> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let known_country_codes = load_known_country_codes(&state.pool).await?;
    let groups = validate_import_pattern_groups(payload.groups, &known_country_codes)?;
    let saved = save_pattern_groups_batch(&state.pool, groups).await?;
    spawn_admin_reindex(state.clone());
    Ok(Json(saved))
}

async fn update_pattern_group(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<AdminPatternGroupPayload>,
) -> ApiResult<AdminPatternGroupResponse> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let group = save_pattern_group(&state.pool, Some(id), payload).await?;
    spawn_admin_reindex(state.clone());
    Ok(Json(group))
}

async fn delete_pattern_group(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    sqlx::query("DELETE FROM admin_search_pattern_groups WHERE id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    spawn_admin_reindex(state.clone());
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_all_pattern_groups(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    sqlx::query("DELETE FROM admin_search_pattern_groups")
        .execute(&state.pool)
        .await?;
    spawn_admin_reindex(state.clone());
    Ok(StatusCode::NO_CONTENT)
}

async fn test_patterns(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<AdminSearchTestPayload>,
) -> ApiResult<AdminSearchTestResponse> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;
    let groups = rules::load_compiled_rules(&state.pool).await?;
    let evaluated = rules::evaluate_patterns(
        &groups,
        rules::AdminSearchEvaluationInput {
            channel_name: payload.channel_name.as_deref(),
            category_name: payload.category_name.as_deref(),
            program_title: payload.program_title.as_deref(),
        },
    );

    Ok(Json(AdminSearchTestResponse {
        country_code: evaluated.country_code,
        provider_name: evaluated.provider_name,
        is_ppv: evaluated.is_ppv,
        is_vip: evaluated.is_vip,
        force_has_epg: evaluated.force_has_epg,
    }))
}

async fn test_search_query(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
    Json(payload): Json<AdminSearchQueryTestPayload>,
) -> ApiResult<AdminSearchQueryTestResponse> {
    require_admin(&state, &jar)?;
    validate_admin_csrf(&jar, &headers)?;

    let parsed = lexicon::parse_search_query(&payload.query);

    Ok(Json(AdminSearchQueryTestResponse {
        search: parsed.search,
        countries: parsed.countries,
        providers: parsed.providers,
        ppv: parsed.ppv,
        vip: parsed.vip,
        require_epg: parsed.require_epg,
    }))
}

async fn save_pattern_group(
    pool: &PgPool,
    id: Option<Uuid>,
    payload: AdminPatternGroupPayload,
) -> Result<AdminPatternGroupResponse, AppError> {
    let value = payload.value.trim();
    if value.is_empty() {
        return Err(AppError::BadRequest("Value is required".to_string()));
    }

    let patterns = rules::parse_patterns_text(&payload.patterns_text);
    if patterns.is_empty() {
        return Err(AppError::BadRequest(
            "At least one pattern is required".to_string(),
        ));
    }

    let normalized_value = rules::normalize_rule_value(payload.kind, value);
    let kind = pattern_kind_as_str(payload.kind);
    let match_target = match_target_as_str(payload.match_target);
    let match_mode = match_mode_as_str(payload.match_mode);
    let known_country_codes = load_known_country_codes(pool).await?;
    let country_codes = validate_country_codes(
        payload.kind,
        rules::parse_country_codes_text(&payload.country_codes_text),
        &known_country_codes,
    )?;

    let mut tx = pool.begin().await?;
    let group_id = if let Some(id) = id {
        sqlx::query(
            r#"
            UPDATE admin_search_pattern_groups
            SET
              kind = $2,
              value = $3,
              normalized_value = $4,
              match_target = $5,
              match_mode = $6,
              priority = $7,
              enabled = $8,
              updated_at = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(kind)
        .bind(value)
        .bind(&normalized_value)
        .bind(match_target)
        .bind(match_mode)
        .bind(payload.priority)
        .bind(payload.enabled)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM admin_search_patterns WHERE group_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM admin_search_provider_countries WHERE group_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        id
    } else {
        sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO admin_search_pattern_groups (
              kind,
              value,
              normalized_value,
              match_target,
              match_mode,
              priority,
              enabled
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(kind)
        .bind(value)
        .bind(&normalized_value)
        .bind(match_target)
        .bind(match_mode)
        .bind(payload.priority)
        .bind(payload.enabled)
        .fetch_one(&mut *tx)
        .await?
    };

    for pattern in &patterns {
        sqlx::query(
            r#"
            INSERT INTO admin_search_patterns (group_id, pattern, normalized_pattern)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(group_id)
        .bind(pattern)
        .bind(rules::normalize_rule_pattern(pattern))
        .execute(&mut *tx)
        .await?;
    }

    for country_code in &country_codes {
        sqlx::query(
            r#"
            INSERT INTO admin_search_provider_countries (group_id, country_code)
            VALUES ($1, $2)
            "#,
        )
        .bind(group_id)
        .bind(country_code)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    let group = rules::load_pattern_groups(pool)
        .await?
        .into_iter()
        .find(|group| group.id == group_id)
        .ok_or_else(|| {
            AppError::NotFound("Pattern group was not found after saving".to_string())
        })?;

    Ok(map_pattern_group_response(group))
}

async fn save_pattern_groups_batch(
    pool: &PgPool,
    groups: Vec<ValidatedPatternGroupInput>,
) -> Result<Vec<AdminPatternGroupResponse>, AppError> {
    if groups.is_empty() {
        return Err(AppError::BadRequest(
            "At least one pattern group is required".to_string(),
        ));
    }

    let mut tx = pool.begin().await?;
    let mut group_ids = Vec::with_capacity(groups.len());

    for group in groups {
        let normalized_value = rules::normalize_rule_value(group.kind, &group.value);
        let group_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            INSERT INTO admin_search_pattern_groups (
              kind,
              value,
              normalized_value,
              match_target,
              match_mode,
              priority,
              enabled
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id
            "#,
        )
        .bind(pattern_kind_as_str(group.kind))
        .bind(&group.value)
        .bind(&normalized_value)
        .bind(match_target_as_str(group.match_target))
        .bind(match_mode_as_str(group.match_mode))
        .bind(group.priority)
        .bind(group.enabled)
        .fetch_one(&mut *tx)
        .await?;

        for pattern in &group.patterns {
            sqlx::query(
                r#"
                INSERT INTO admin_search_patterns (group_id, pattern, normalized_pattern)
                VALUES ($1, $2, $3)
                "#,
            )
            .bind(group_id)
            .bind(pattern)
            .bind(rules::normalize_rule_pattern(pattern))
            .execute(&mut *tx)
            .await?;
        }

        for country_code in &group.country_codes {
            sqlx::query(
                r#"
                INSERT INTO admin_search_provider_countries (group_id, country_code)
                VALUES ($1, $2)
                "#,
            )
            .bind(group_id)
            .bind(country_code)
            .execute(&mut *tx)
            .await?;
        }

        group_ids.push(group_id);
    }

    tx.commit().await?;

    let groups = rules::load_pattern_groups(pool).await?;
    let groups_by_id = groups
        .into_iter()
        .map(|group| (group.id, group))
        .collect::<HashMap<_, _>>();

    group_ids
        .into_iter()
        .map(|id| {
            groups_by_id
                .get(&id)
                .cloned()
                .map(map_pattern_group_response)
                .ok_or_else(|| {
                    AppError::NotFound("Imported pattern group was not found".to_string())
                })
        })
        .collect()
}

fn map_pattern_group_response(group: rules::LoadedAdminPatternGroup) -> AdminPatternGroupResponse {
    let patterns = group
        .patterns
        .into_iter()
        .map(|pattern| AdminPatternResponse {
            id: pattern.id,
            pattern: pattern.pattern,
        })
        .collect::<Vec<_>>();

    AdminPatternGroupResponse {
        id: group.id,
        kind: group.kind,
        value: group.value,
        normalized_value: group.normalized_value,
        match_target: group.match_target,
        match_mode: group.match_mode,
        priority: group.priority,
        enabled: group.enabled,
        patterns_text: patterns
            .iter()
            .map(|pattern| pattern.pattern.clone())
            .collect::<Vec<_>>()
            .join(","),
        country_codes_text: group.country_codes.join(","),
        country_codes: group.country_codes,
        patterns,
    }
}

fn validate_import_pattern_groups(
    groups: Vec<AdminPatternGroupImportItem>,
    known_country_codes: &HashSet<String>,
) -> Result<Vec<ValidatedPatternGroupInput>, AppError> {
    if groups.is_empty() {
        return Err(AppError::BadRequest(
            "At least one pattern group is required".to_string(),
        ));
    }

    let mut errors = Vec::new();
    let mut validated = Vec::with_capacity(groups.len());
    let mut available_country_codes = known_country_codes.clone();

    for group in &groups {
        if group.kind.trim().eq_ignore_ascii_case("country") {
            let value = group.value.trim();
            if !value.is_empty() {
                available_country_codes.insert(value.to_ascii_lowercase());
            }
        }
    }

    for (index, group) in groups.into_iter().enumerate() {
        match validate_import_pattern_group(index, group, &available_country_codes) {
            Ok(group) => validated.push(group),
            Err(mut group_errors) => errors.append(&mut group_errors),
        }
    }

    if !errors.is_empty() {
        return Err(AppError::BadRequestDetailed {
            message: "Import payload contains invalid pattern groups".to_string(),
            details: serde_json::json!(errors),
        });
    }

    Ok(validated)
}

fn validate_import_pattern_group(
    index: usize,
    group: AdminPatternGroupImportItem,
    known_country_codes: &HashSet<String>,
) -> Result<ValidatedPatternGroupInput, Vec<AdminPatternGroupImportErrorDetail>> {
    let mut errors = Vec::new();

    let kind = match parse_import_pattern_kind(&group.kind) {
        Ok(kind) => Some(kind),
        Err(message) => {
            errors.push(import_error(index, "kind", &message));
            None
        }
    };

    let match_target = match parse_import_match_target(&group.match_target) {
        Ok(match_target) => Some(match_target),
        Err(message) => {
            errors.push(import_error(index, "matchTarget", &message));
            None
        }
    };

    let match_mode = match parse_import_match_mode(&group.match_mode) {
        Ok(match_mode) => Some(match_mode),
        Err(message) => {
            errors.push(import_error(index, "matchMode", &message));
            None
        }
    };

    let value = group.value.trim().to_string();
    if value.is_empty() {
        errors.push(import_error(index, "value", "Value is required"));
    }

    let patterns = normalize_import_patterns(group.patterns, group.patterns_text);
    if patterns.is_empty() {
        errors.push(import_error(
            index,
            "patterns",
            "At least one pattern is required",
        ));
    }

    let country_codes = normalize_import_country_codes(group.country_codes);
    let Some(kind) = kind else {
        if errors.is_empty() {
            return Ok(ValidatedPatternGroupInput {
                kind: AdminSearchPatternKind::Country,
                value,
                match_target: match_target.expect("validated match target"),
                match_mode: match_mode.expect("validated match mode"),
                priority: group.priority.unwrap_or(0),
                enabled: group.enabled.unwrap_or(true),
                patterns,
                country_codes: Vec::new(),
            });
        }
        return Err(errors);
    };

    match validate_country_codes_import(index, kind, country_codes, known_country_codes) {
        Ok(country_codes) => {
            if !errors.is_empty() {
                return Err(errors);
            }

            return Ok(ValidatedPatternGroupInput {
                kind,
                value,
                match_target: match_target.expect("validated match target"),
                match_mode: match_mode.expect("validated match mode"),
                priority: group.priority.unwrap_or(0),
                enabled: group.enabled.unwrap_or(true),
                patterns,
                country_codes,
            });
        }
        Err(mut country_errors) => errors.append(&mut country_errors),
    }

    if !errors.is_empty() {
        return Err(errors);
    }

    unreachable!("validated import groups must have returned or failed by now")
}

fn normalize_import_patterns(
    patterns: Option<Vec<String>>,
    patterns_text: Option<String>,
) -> Vec<String> {
    if let Some(patterns) = patterns {
        let mut seen = HashSet::new();
        return patterns
            .into_iter()
            .map(|pattern| pattern.trim().to_string())
            .filter(|pattern| !pattern.is_empty())
            .filter_map(|pattern| {
                let key = pattern.to_ascii_lowercase();
                seen.insert(key).then_some(pattern)
            })
            .collect();
    }

    patterns_text
        .as_deref()
        .map(rules::parse_patterns_text)
        .unwrap_or_default()
}

fn normalize_import_country_codes(country_codes: Option<Vec<String>>) -> Vec<String> {
    let mut seen = HashSet::new();
    country_codes
        .unwrap_or_default()
        .into_iter()
        .map(|country_code| country_code.trim().to_ascii_lowercase())
        .filter(|country_code| !country_code.is_empty())
        .filter_map(|country_code| seen.insert(country_code.clone()).then_some(country_code))
        .collect()
}

async fn load_known_country_codes(pool: &PgPool) -> Result<HashSet<String>, AppError> {
    let country_codes = sqlx::query_scalar::<_, String>(
        r#"
        SELECT DISTINCT normalized_value
        FROM admin_search_pattern_groups
        WHERE kind = 'country'
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(country_codes.into_iter().collect())
}

fn validate_country_codes(
    kind: AdminSearchPatternKind,
    country_codes: Vec<String>,
    known_country_codes: &HashSet<String>,
) -> Result<Vec<String>, AppError> {
    if kind != AdminSearchPatternKind::Provider {
        return Ok(Vec::new());
    }

    if country_codes.is_empty() {
        return Err(AppError::BadRequest(
            "Provider rules require at least one related country code".to_string(),
        ));
    }

    let invalid = country_codes
        .iter()
        .filter(|country_code| !known_country_codes.contains(*country_code))
        .cloned()
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err(AppError::BadRequest(format!(
            "Unknown provider country code(s): {}",
            invalid.join(", ")
        )));
    }

    Ok(country_codes)
}

fn validate_country_codes_import(
    index: usize,
    kind: AdminSearchPatternKind,
    country_codes: Vec<String>,
    known_country_codes: &HashSet<String>,
) -> Result<Vec<String>, Vec<AdminPatternGroupImportErrorDetail>> {
    if kind != AdminSearchPatternKind::Provider {
        return Ok(Vec::new());
    }

    let mut errors = Vec::new();
    if country_codes.is_empty() {
        errors.push(import_error(
            index,
            "countryCodes",
            "Provider rules require at least one related country code",
        ));
    }

    let invalid = country_codes
        .iter()
        .filter(|country_code| !known_country_codes.contains(*country_code))
        .cloned()
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        errors.push(import_error(
            index,
            "countryCodes",
            &format!("Unknown country code(s): {}", invalid.join(", ")),
        ));
    }

    if errors.is_empty() {
        Ok(country_codes)
    } else {
        Err(errors)
    }
}

fn parse_import_pattern_kind(value: &str) -> Result<AdminSearchPatternKind, String> {
    match value.trim() {
        "country" => Ok(AdminSearchPatternKind::Country),
        "provider" => Ok(AdminSearchPatternKind::Provider),
        "flag" => Ok(AdminSearchPatternKind::Flag),
        _ => Err("Kind must be one of: country, provider, flag".to_string()),
    }
}

fn parse_import_match_target(value: &str) -> Result<AdminSearchMatchTarget, String> {
    match value.trim() {
        "channel_name" => Ok(AdminSearchMatchTarget::ChannelName),
        "category_name" => Ok(AdminSearchMatchTarget::CategoryName),
        "program_title" => Ok(AdminSearchMatchTarget::ProgramTitle),
        "channel_or_category" => Ok(AdminSearchMatchTarget::ChannelOrCategory),
        "any_text" => Ok(AdminSearchMatchTarget::AnyText),
        _ => Err(
            "Match target must be one of: channel_name, category_name, program_title, channel_or_category, any_text"
                .to_string(),
        ),
    }
}

fn parse_import_match_mode(value: &str) -> Result<AdminSearchMatchMode, String> {
    match value.trim() {
        "prefix" => Ok(AdminSearchMatchMode::Prefix),
        "contains" => Ok(AdminSearchMatchMode::Contains),
        "exact" => Ok(AdminSearchMatchMode::Exact),
        _ => Err("Match mode must be one of: prefix, contains, exact".to_string()),
    }
}

fn import_error(index: usize, field: &str, message: &str) -> AdminPatternGroupImportErrorDetail {
    AdminPatternGroupImportErrorDetail {
        index,
        field: field.to_string(),
        message: message.to_string(),
    }
}

fn require_admin(state: &AppState, jar: &CookieJar) -> Result<(), AppError> {
    if state.config.admin_password.is_none() {
        return Err(AppError::Unauthorized);
    }

    let token = jar
        .get(ADMIN_SESSION_COOKIE_NAME)
        .map(|cookie| cookie.value())
        .ok_or(AppError::Unauthorized)?;

    let claims = decode::<AdminAccessClaims>(
        token,
        &DecodingKey::from_secret(state.config.jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| AppError::Unauthorized)?
    .claims;

    if claims.role != "admin" {
        return Err(AppError::Unauthorized);
    }

    Ok(())
}

fn validate_admin_csrf(jar: &CookieJar, headers: &HeaderMap) -> Result<(), AppError> {
    let csrf_cookie = jar
        .get(ADMIN_CSRF_COOKIE_NAME)
        .ok_or(AppError::Unauthorized)?;
    let csrf_header = headers
        .get(HeaderName::from_static(CSRF_HEADER_NAME))
        .and_then(|value| value.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    if csrf_cookie.value() != csrf_header {
        return Err(AppError::Unauthorized);
    }
    Ok(())
}

fn set_admin_auth_cookies(
    state: &AppState,
    jar: CookieJar,
    session_token: String,
    csrf_token: String,
) -> CookieJar {
    jar.add(build_admin_session_cookie(state, session_token))
        .add(build_admin_csrf_cookie(state, csrf_token))
}

fn clear_admin_auth_cookies(state: &AppState, jar: CookieJar) -> CookieJar {
    jar.add(expired_admin_session_cookie(state))
        .add(expired_admin_csrf_cookie(state))
}

fn build_admin_session_cookie(state: &AppState, value: String) -> Cookie<'static> {
    let mut builder = Cookie::build((ADMIN_SESSION_COOKIE_NAME, value))
        .http_only(true)
        .path("/api/admin")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::days(state.config.refresh_token_days));
    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }
    builder.build()
}

fn build_admin_csrf_cookie(state: &AppState, value: String) -> Cookie<'static> {
    let mut builder = Cookie::build((ADMIN_CSRF_COOKIE_NAME, value))
        .http_only(false)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::days(state.config.refresh_token_days));
    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }
    builder.build()
}

fn expired_admin_session_cookie(state: &AppState) -> Cookie<'static> {
    let mut builder = Cookie::build((ADMIN_SESSION_COOKIE_NAME, ""))
        .http_only(true)
        .path("/api/admin")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(0));
    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }
    builder.build()
}

fn expired_admin_csrf_cookie(state: &AppState) -> Cookie<'static> {
    let mut builder = Cookie::build((ADMIN_CSRF_COOKIE_NAME, ""))
        .http_only(false)
        .path("/")
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(0));
    if state.config.browser_cookie_secure {
        builder = builder.secure(true);
    }
    builder.build()
}

fn pattern_kind_as_str(value: AdminSearchPatternKind) -> &'static str {
    match value {
        AdminSearchPatternKind::Country => "country",
        AdminSearchPatternKind::Provider => "provider",
        AdminSearchPatternKind::Flag => "flag",
    }
}

fn match_target_as_str(value: AdminSearchMatchTarget) -> &'static str {
    match value {
        AdminSearchMatchTarget::ChannelName => "channel_name",
        AdminSearchMatchTarget::CategoryName => "category_name",
        AdminSearchMatchTarget::ProgramTitle => "program_title",
        AdminSearchMatchTarget::ChannelOrCategory => "channel_or_category",
        AdminSearchMatchTarget::AnyText => "any_text",
    }
}

fn match_mode_as_str(value: AdminSearchMatchMode) -> &'static str {
    match value {
        AdminSearchMatchMode::Prefix => "prefix",
        AdminSearchMatchMode::Contains => "contains",
        AdminSearchMatchMode::Exact => "exact",
    }
}

fn spawn_admin_reindex(state: AppState) {
    tokio::spawn(async move {
        let user_ids = match sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT DISTINCT user_id
            FROM provider_profiles
            ORDER BY user_id ASC
            "#,
        )
        .fetch_all(&state.pool)
        .await
        {
            Ok(user_ids) => user_ids,
            Err(error) => {
                warn!("failed to load users for admin search reindex: {error:?}");
                return;
            }
        };

        for user_id in user_ids {
            if let Err(error) = search::indexing::rebuild_search_documents(&state, user_id).await {
                warn!(user_id = %user_id, "failed to rebuild search documents after admin rule change: {error:?}");
                continue;
            }

            if let Some(meili) = state.meili.as_ref() {
                if let Err(error) =
                    search::indexing::rebuild_meili_indexes(&state, meili, user_id, None).await
                {
                    warn!(user_id = %user_id, "failed to rebuild Meilisearch indexes after admin rule change: {error:?}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_import_pattern_groups_accepts_valid_batches_with_defaults() {
        let known_country_codes = HashSet::from(["se".to_string()]);
        let groups = validate_import_pattern_groups(
            vec![AdminPatternGroupImportItem {
                kind: "country".to_string(),
                value: "se".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "prefix".to_string(),
                priority: None,
                enabled: None,
                patterns: Some(vec!["SE:".to_string(), "SE|".to_string()]),
                patterns_text: None,
                country_codes: None,
            }],
            &known_country_codes,
        )
        .expect("expected valid import groups");

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].priority, 0);
        assert!(groups[0].enabled);
        assert_eq!(
            groups[0].patterns,
            vec!["SE:".to_string(), "SE|".to_string()]
        );
    }

    #[test]
    fn validate_import_pattern_groups_rejects_items_without_patterns() {
        let known_country_codes = HashSet::new();
        let error = validate_import_pattern_groups(
            vec![AdminPatternGroupImportItem {
                kind: "flag".to_string(),
                value: "ppv".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "contains".to_string(),
                priority: Some(10),
                enabled: Some(true),
                patterns: Some(vec![]),
                patterns_text: None,
                country_codes: None,
            }],
            &known_country_codes,
        )
        .expect_err("expected invalid import to fail");

        match error {
            AppError::BadRequestDetailed { details, .. } => {
                let details = details
                    .as_array()
                    .expect("expected array of validation errors");
                assert_eq!(details.len(), 1);
                assert_eq!(details[0]["index"], 0);
                assert_eq!(details[0]["field"], "patterns");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn validate_import_pattern_groups_reports_invalid_enum_values() {
        let known_country_codes = HashSet::new();
        let error = validate_import_pattern_groups(
            vec![AdminPatternGroupImportItem {
                kind: "region".to_string(),
                value: "se".to_string(),
                match_target: "channel".to_string(),
                match_mode: "wildcard".to_string(),
                priority: Some(1),
                enabled: Some(true),
                patterns: Some(vec!["SE:".to_string()]),
                patterns_text: None,
                country_codes: None,
            }],
            &known_country_codes,
        )
        .expect_err("expected invalid enum values to fail");

        match error {
            AppError::BadRequestDetailed { details, .. } => {
                let details = details
                    .as_array()
                    .expect("expected array of validation errors");
                assert_eq!(details.len(), 3);
                assert!(details.iter().any(|detail| detail["field"] == "kind"));
                assert!(
                    details
                        .iter()
                        .any(|detail| detail["field"] == "matchTarget")
                );
                assert!(details.iter().any(|detail| detail["field"] == "matchMode"));
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn normalize_import_patterns_supports_patterns_text_fallback() {
        let patterns = normalize_import_patterns(None, Some("SE:, SE|, se:".to_string()));
        assert_eq!(patterns, vec!["SE:".to_string(), "SE|".to_string()]);
    }

    #[test]
    fn validate_import_pattern_groups_accepts_provider_country_codes() {
        let known_country_codes = HashSet::from(["se".to_string(), "uk".to_string()]);
        let groups = validate_import_pattern_groups(
            vec![AdminPatternGroupImportItem {
                kind: "provider".to_string(),
                value: "viaplay".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "contains".to_string(),
                priority: Some(10),
                enabled: Some(true),
                patterns: Some(vec!["VIAPLAY".to_string()]),
                patterns_text: None,
                country_codes: Some(vec!["SE".to_string(), "uk".to_string()]),
            }],
            &known_country_codes,
        )
        .expect("expected provider import groups");

        assert_eq!(
            groups[0].country_codes,
            vec!["se".to_string(), "uk".to_string()]
        );
    }

    #[test]
    fn validate_import_pattern_groups_rejects_unknown_provider_country_codes() {
        let known_country_codes = HashSet::from(["se".to_string()]);
        let error = validate_import_pattern_groups(
            vec![AdminPatternGroupImportItem {
                kind: "provider".to_string(),
                value: "viaplay".to_string(),
                match_target: "channel_or_category".to_string(),
                match_mode: "contains".to_string(),
                priority: Some(10),
                enabled: Some(true),
                patterns: Some(vec!["VIAPLAY".to_string()]),
                patterns_text: None,
                country_codes: Some(vec!["uk".to_string()]),
            }],
            &known_country_codes,
        )
        .expect_err("expected invalid provider country code");

        match error {
            AppError::BadRequestDetailed { details, .. } => {
                let details = details
                    .as_array()
                    .expect("expected array of validation errors");
                assert!(
                    details
                        .iter()
                        .any(|detail| detail["field"] == "countryCodes")
                );
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn validate_import_pattern_groups_accepts_provider_country_codes_defined_in_same_batch() {
        let known_country_codes = HashSet::new();
        let groups = validate_import_pattern_groups(
            vec![
                AdminPatternGroupImportItem {
                    kind: "provider".to_string(),
                    value: "viaplay".to_string(),
                    match_target: "channel_or_category".to_string(),
                    match_mode: "contains".to_string(),
                    priority: Some(10),
                    enabled: Some(true),
                    patterns: Some(vec!["VIAPLAY".to_string()]),
                    patterns_text: None,
                    country_codes: Some(vec!["se".to_string(), "uk".to_string()]),
                },
                AdminPatternGroupImportItem {
                    kind: "country".to_string(),
                    value: "se".to_string(),
                    match_target: "channel_or_category".to_string(),
                    match_mode: "prefix".to_string(),
                    priority: Some(10),
                    enabled: Some(true),
                    patterns: Some(vec!["SE:".to_string()]),
                    patterns_text: None,
                    country_codes: None,
                },
                AdminPatternGroupImportItem {
                    kind: "country".to_string(),
                    value: "uk".to_string(),
                    match_target: "channel_or_category".to_string(),
                    match_mode: "prefix".to_string(),
                    priority: Some(10),
                    enabled: Some(true),
                    patterns: Some(vec!["UK:".to_string()]),
                    patterns_text: None,
                    country_codes: None,
                },
            ],
            &known_country_codes,
        )
        .expect("expected same-batch countries to satisfy provider country validation");

        assert_eq!(
            groups[0].country_codes,
            vec!["se".to_string(), "uk".to_string()]
        );
    }
}
