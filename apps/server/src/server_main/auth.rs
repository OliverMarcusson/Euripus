use super::*;

pub(super) fn browser_router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh_session))
        .route("/auth/logout", post(logout))
}

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", delete(revoke_session))
}

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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CredentialsPayload {
    username: String,
    password: String,
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
    expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
}

async fn register(
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

async fn login(
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

async fn refresh_session(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: HeaderMap,
) -> Result<(CookieJar, Json<AuthSessionResponse>), AppError> {
    validate_browser_csrf(&jar, &headers)?;
    let refresh_token = read_browser_refresh_token(&jar)?;
    let session = refresh_session_from_token(&state, &refresh_token).await?;
    Ok(browser_auth_response(&state, jar, session))
}

async fn logout(
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
    state.session_cache.remove(&(id, auth.user_id));
    Ok(StatusCode::NO_CONTENT)
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
        RETURNING id, user_id, expires_at, revoked_at
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
    state.session_cache.remove(&(session.id, session.user_id));

    issue_session(state, &user, session.id, next_refresh_token)
}

async fn get_valid_session_by_refresh_token(
    state: &AppState,
    refresh_token: &str,
) -> Result<SessionRecord, AppError> {
    let refresh_hash = hash_refresh_token(refresh_token);
    let session = sqlx::query_as::<_, SessionRecord>(
        r#"
        SELECT id, user_id, expires_at, revoked_at
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
    let session = get_valid_session_by_refresh_token(state, refresh_token).await?;
    let refresh_hash = hash_refresh_token(refresh_token);
    sqlx::query("UPDATE sessions SET revoked_at = NOW() WHERE refresh_token_hash = $1")
        .bind(refresh_hash)
        .execute(&state.pool)
        .await?;
    state.session_cache.remove(&(session.id, session.user_id));

    Ok(())
}

fn issue_session(
    state: &AppState,
    user: &UserRecord,
    session_id: Uuid,
    refresh_token: String,
) -> Result<IssuedSession, AppError> {
    let (access_token, access_expires_at) =
        create_access_token(state, user.id, &user.username, session_id)?;
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
