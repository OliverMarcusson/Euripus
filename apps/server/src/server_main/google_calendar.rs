use super::*;
use axum::response::Redirect;

const CALENDAR_EVENTS_SCOPE: &str = "https://www.googleapis.com/auth/calendar.events";
const CALENDAR_LIST_SCOPE: &str = "https://www.googleapis.com/auth/calendar.calendarlist.readonly";
const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route("/integrations/google-calendar/status", get(get_status))
        .route("/integrations/google-calendar/connect", post(connect))
        .route("/integrations/google-calendar/callback", get(callback))
        .route(
            "/integrations/google-calendar/calendars",
            get(list_calendars),
        )
        .route(
            "/integrations/google-calendar/calendar",
            put(select_calendar),
        )
        .route("/integrations/google-calendar", delete(disconnect))
        .route("/sports/events/{id}/calendar", post(add_sports_event))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionStatusResponse {
    configured: bool,
    connected: bool,
    needs_reauthorization: bool,
    selected_calendar_id: Option<String>,
    selected_calendar_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConnectResponse {
    authorization_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CalendarResponse {
    id: String,
    summary: String,
    primary: bool,
    access_role: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AddEventResponse {
    google_event_id: String,
    html_link: Option<String>,
    created: bool,
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    state: String,
    code: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CalendarSelectionPayload {
    calendar_id: String,
}

#[derive(Debug, FromRow)]
struct ConnectionRecord {
    access_token_encrypted: String,
    refresh_token_encrypted: String,
    token_expires_at: DateTime<Utc>,
    selected_calendar_id: Option<String>,
    needs_reauthorization: bool,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
    refresh_token: Option<String>,
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarListResponse {
    #[serde(default)]
    items: Vec<GoogleCalendarEntry>,
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleCalendarEntry {
    id: String,
    summary: String,
    #[serde(default)]
    primary: bool,
    access_role: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoogleEventResponse {
    id: String,
    html_link: Option<String>,
}

#[derive(Debug, FromRow)]
struct ExportRecord {
    google_event_id: String,
}

fn is_configured(state: &AppState) -> bool {
    state.config.google_client_id.is_some()
        && state.config.google_client_secret.is_some()
        && state.config.google_calendar_redirect_url.is_some()
}

fn oauth_config(state: &AppState) -> Result<(&str, &str, &Url), AppError> {
    match (
        state.config.google_client_id.as_deref(),
        state.config.google_client_secret.as_deref(),
        state.config.google_calendar_redirect_url.as_ref(),
    ) {
        (Some(id), Some(secret), Some(redirect)) => Ok((id, secret, redirect)),
        _ => Err(AppError::ServiceUnavailable(
            "Google Calendar is not configured on this Euripus server".to_string(),
        )),
    }
}

async fn get_status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<ConnectionStatusResponse> {
    let auth = require_auth(&state, &headers).await?;
    let row = sqlx::query_as::<_, (bool, Option<String>, Option<String>)>(
        r#"
        SELECT needs_reauthorization, selected_calendar_id, selected_calendar_name
        FROM google_calendar_connections WHERE user_id = $1
        "#,
    )
    .bind(auth.user_id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(Json(ConnectionStatusResponse {
        configured: is_configured(&state),
        connected: row.is_some(),
        needs_reauthorization: row.as_ref().is_some_and(|value| value.0),
        selected_calendar_id: row.as_ref().and_then(|value| value.1.clone()),
        selected_calendar_name: row.and_then(|value| value.2),
    }))
}

async fn connect(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<ConnectResponse> {
    let auth = require_auth(&state, &headers).await?;
    let (client_id, _, redirect_url) = oauth_config(&state)?;
    let state_token = random_urlsafe(32);
    let verifier = random_urlsafe(64);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state_hash = hex_encode(&Sha256::digest(state_token.as_bytes()));
    let encrypted_verifier = encrypt_secret(&state.config.encryption_key, &verifier)?;

    sqlx::query(
        "DELETE FROM google_calendar_oauth_states WHERE expires_at <= NOW() OR user_id = $1",
    )
    .bind(auth.user_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO google_calendar_oauth_states
          (state_hash, user_id, pkce_verifier_encrypted, expires_at)
        VALUES ($1, $2, $3, NOW() + INTERVAL '10 minutes')
        "#,
    )
    .bind(state_hash)
    .bind(auth.user_id)
    .bind(encrypted_verifier)
    .execute(&state.pool)
    .await?;

    let mut authorization_url =
        Url::parse(GOOGLE_AUTH_URL).map_err(|error| AppError::Internal(anyhow!(error)))?;
    authorization_url
        .query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_url.as_str())
        .append_pair("response_type", "code")
        .append_pair(
            "scope",
            &format!("{CALENDAR_EVENTS_SCOPE} {CALENDAR_LIST_SCOPE}"),
        )
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent")
        .append_pair("include_granted_scopes", "true")
        .append_pair("state", &state_token)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256");

    Ok(Json(ConnectResponse {
        authorization_url: authorization_url.to_string(),
    }))
}

async fn callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Redirect, AppError> {
    let (_, _, redirect_url) = oauth_config(&state)?;
    let state_hash = hex_encode(&Sha256::digest(query.state.as_bytes()));
    let row = sqlx::query_as::<_, (Uuid, String)>(
        r#"
        DELETE FROM google_calendar_oauth_states
        WHERE state_hash = $1 AND expires_at > NOW()
        RETURNING user_id, pkce_verifier_encrypted
        "#,
    )
    .bind(state_hash)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Google OAuth state is invalid or expired".to_string()))?;

    let settings_url = state
        .config
        .public_origin
        .as_ref()
        .ok_or_else(|| {
            AppError::ServiceUnavailable(
                "APP_PUBLIC_ORIGIN is required for Google Calendar".to_string(),
            )
        })?
        .join("/settings")
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    if let Some(error) = query.error {
        let mut url = settings_url;
        url.query_pairs_mut().append_pair("googleCalendar", &error);
        return Ok(Redirect::to(url.as_str()));
    }
    let code = query.code.ok_or_else(|| {
        AppError::BadRequest("Google did not return an authorization code".to_string())
    })?;
    let verifier = decrypt_secret(&state.config.encryption_key, &row.1)?;
    let (client_id, client_secret, _) = oauth_config(&state)?;
    let response = state
        .provider_http_client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("code", code.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect_url.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|error| AppError::BadGateway(format!("Google token exchange failed: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "Google token exchange failed with status {}",
            response.status()
        )));
    }
    let token = response.json::<TokenResponse>().await.map_err(|error| {
        AppError::BadGateway(format!(
            "Google returned an invalid token response: {error}"
        ))
    })?;
    let refresh_token = token.refresh_token.ok_or_else(|| {
        AppError::BadGateway(
            "Google did not provide offline calendar access; reconnect and grant consent"
                .to_string(),
        )
    })?;
    let access_encrypted = encrypt_secret(&state.config.encryption_key, &token.access_token)?;
    let refresh_encrypted = encrypt_secret(&state.config.encryption_key, &refresh_token)?;
    let scopes = token
        .scope
        .unwrap_or_default()
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    sqlx::query(
        r#"
        INSERT INTO google_calendar_connections
          (user_id, access_token_encrypted, refresh_token_encrypted, token_expires_at, granted_scopes)
        VALUES ($1, $2, $3, NOW() + ($4 * INTERVAL '1 second'), $5)
        ON CONFLICT (user_id) DO UPDATE SET
          access_token_encrypted = EXCLUDED.access_token_encrypted,
          refresh_token_encrypted = EXCLUDED.refresh_token_encrypted,
          token_expires_at = EXCLUDED.token_expires_at,
          granted_scopes = EXCLUDED.granted_scopes,
          needs_reauthorization = FALSE,
          updated_at = NOW()
        "#,
    )
    .bind(row.0)
    .bind(access_encrypted)
    .bind(refresh_encrypted)
    .bind(token.expires_in)
    .bind(scopes)
    .execute(&state.pool)
    .await?;

    let mut url = settings_url;
    url.query_pairs_mut()
        .append_pair("googleCalendar", "connected");
    Ok(Redirect::to(url.as_str()))
}

async fn load_connection(state: &AppState, user_id: Uuid) -> Result<ConnectionRecord, AppError> {
    sqlx::query_as::<_, ConnectionRecord>(
        r#"
        SELECT access_token_encrypted, refresh_token_encrypted, token_expires_at,
          selected_calendar_id, needs_reauthorization
        FROM google_calendar_connections WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("Connect Google Calendar first".to_string()))
}

async fn access_token(state: &AppState, user_id: Uuid) -> Result<String, AppError> {
    let connection = load_connection(state, user_id).await?;
    if connection.needs_reauthorization {
        return Err(AppError::Unauthorized);
    }
    if connection.token_expires_at > Utc::now() + ChronoDuration::seconds(60) {
        return decrypt_secret(
            &state.config.encryption_key,
            &connection.access_token_encrypted,
        )
        .map_err(AppError::Internal);
    }
    let (client_id, client_secret, _) = oauth_config(state)?;
    let refresh_token = decrypt_secret(
        &state.config.encryption_key,
        &connection.refresh_token_encrypted,
    )?;
    let response = state
        .provider_http_client
        .post(GOOGLE_TOKEN_URL)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(|error| AppError::BadGateway(format!("Google token refresh failed: {error}")))?;
    if !response.status().is_success() {
        let status = response.status();
        if status == StatusCode::BAD_REQUEST || status == StatusCode::UNAUTHORIZED {
            sqlx::query("UPDATE google_calendar_connections SET needs_reauthorization = TRUE, updated_at = NOW() WHERE user_id = $1")
                .bind(user_id)
                .execute(&state.pool)
                .await?;
            return Err(AppError::Unauthorized);
        }
        return Err(AppError::BadGateway(format!(
            "Google token refresh failed with status {status}"
        )));
    }
    let token = response.json::<TokenResponse>().await.map_err(|error| {
        AppError::BadGateway(format!(
            "Google returned an invalid refresh response: {error}"
        ))
    })?;
    let encrypted = encrypt_secret(&state.config.encryption_key, &token.access_token)?;
    sqlx::query(
        "UPDATE google_calendar_connections SET access_token_encrypted = $2, token_expires_at = NOW() + ($3 * INTERVAL '1 second'), needs_reauthorization = FALSE, updated_at = NOW() WHERE user_id = $1",
    )
    .bind(user_id)
    .bind(encrypted)
    .bind(token.expires_in)
    .execute(&state.pool)
    .await?;
    Ok(token.access_token)
}

async fn fetch_calendars(
    state: &AppState,
    user_id: Uuid,
) -> Result<Vec<GoogleCalendarEntry>, AppError> {
    let token = access_token(state, user_id).await?;
    let mut calendars = Vec::new();
    let mut page_token: Option<String> = None;
    loop {
        let mut url = Url::parse("https://www.googleapis.com/calendar/v3/users/me/calendarList")
            .map_err(|error| AppError::Internal(anyhow!(error)))?;
        url.query_pairs_mut()
            .append_pair("minAccessRole", "writer")
            .append_pair("maxResults", "250");
        if let Some(value) = page_token.as_deref() {
            url.query_pairs_mut().append_pair("pageToken", value);
        }
        let response = state
            .provider_http_client
            .get(url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|error| {
                AppError::BadGateway(format!("Google Calendar request failed: {error}"))
            })?;
        if !response.status().is_success() {
            return Err(AppError::BadGateway(format!(
                "Google Calendar list failed with status {}",
                response.status()
            )));
        }
        let page = response
            .json::<GoogleCalendarListResponse>()
            .await
            .map_err(|error| {
                AppError::BadGateway(format!("Google returned an invalid calendar list: {error}"))
            })?;
        calendars.extend(page.items);
        page_token = page.next_page_token;
        if page_token.is_none() {
            break;
        }
    }
    Ok(calendars)
}

async fn list_calendars(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<CalendarResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let calendars = fetch_calendars(&state, auth.user_id).await?;
    Ok(Json(
        calendars
            .into_iter()
            .map(|entry| CalendarResponse {
                id: entry.id,
                summary: entry.summary,
                primary: entry.primary,
                access_role: entry.access_role,
            })
            .collect(),
    ))
}

async fn select_calendar(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CalendarSelectionPayload>,
) -> ApiResult<ConnectionStatusResponse> {
    let auth = require_auth(&state, &headers).await?;
    let calendars = fetch_calendars(&state, auth.user_id).await?;
    let selected = calendars
        .into_iter()
        .find(|entry| entry.id == payload.calendar_id)
        .ok_or_else(|| AppError::BadRequest("Select a writable Google calendar".to_string()))?;
    sqlx::query("UPDATE google_calendar_connections SET selected_calendar_id = $2, selected_calendar_name = $3, updated_at = NOW() WHERE user_id = $1")
        .bind(auth.user_id)
        .bind(&selected.id)
        .bind(&selected.summary)
        .execute(&state.pool)
        .await?;
    Ok(Json(ConnectionStatusResponse {
        configured: true,
        connected: true,
        needs_reauthorization: false,
        selected_calendar_id: Some(selected.id),
        selected_calendar_name: Some(selected.summary),
    }))
}

async fn disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    if let Ok(connection) = load_connection(&state, auth.user_id).await {
        if let Ok(token) = decrypt_secret(
            &state.config.encryption_key,
            &connection.refresh_token_encrypted,
        ) {
            let _ = state
                .provider_http_client
                .post(GOOGLE_REVOKE_URL)
                .form(&[("token", token)])
                .send()
                .await;
        }
    }
    sqlx::query("DELETE FROM google_calendar_connections WHERE user_id = $1")
        .bind(auth.user_id)
        .execute(&state.pool)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn add_sports_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> ApiResult<AddEventResponse> {
    let auth = require_auth(&state, &headers).await?;
    let connection = load_connection(&state, auth.user_id).await?;
    let calendar_id = connection.selected_calendar_id.ok_or_else(|| {
        AppError::BadRequest("Choose a Google calendar in Settings first".to_string())
    })?;
    let event = sports::fetch_event_by_id(&state, &id).await?;
    let token = access_token(&state, auth.user_id).await?;
    let body = calendar_event_body(&state, &event)?;
    let fingerprint = hex_encode(&Sha256::digest(
        serde_json::to_vec(&body).map_err(|error| AppError::Internal(anyhow!(error)))?,
    ));
    let existing = sqlx::query_as::<_, ExportRecord>(
        "SELECT google_event_id FROM sports_calendar_events WHERE user_id = $1 AND sports_event_id = $2 AND calendar_id = $3",
    )
    .bind(auth.user_id)
    .bind(&id)
    .bind(&calendar_id)
    .fetch_optional(&state.pool)
    .await?;

    let (google_event, created) = if let Some(export) = existing {
        let url = google_event_url(&calendar_id, Some(&export.google_event_id))?;
        let response = state
            .provider_http_client
            .patch(url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                AppError::BadGateway(format!("Google Calendar update failed: {error}"))
            })?;
        if response.status() == StatusCode::NOT_FOUND {
            (
                insert_google_event(&state, &token, &calendar_id, &body).await?,
                true,
            )
        } else if response.status().is_success() {
            (
                response
                    .json::<GoogleEventResponse>()
                    .await
                    .map_err(|error| {
                        AppError::BadGateway(format!("Google returned an invalid event: {error}"))
                    })?,
                false,
            )
        } else {
            return Err(AppError::BadGateway(format!(
                "Google Calendar update failed with status {}",
                response.status()
            )));
        }
    } else {
        (
            insert_google_event(&state, &token, &calendar_id, &body).await?,
            true,
        )
    };

    sqlx::query(
        r#"
        INSERT INTO sports_calendar_events
          (user_id, sports_event_id, calendar_id, google_event_id, google_event_url, event_fingerprint)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (user_id, sports_event_id, calendar_id) DO UPDATE SET
          google_event_id = EXCLUDED.google_event_id,
          google_event_url = EXCLUDED.google_event_url,
          event_fingerprint = EXCLUDED.event_fingerprint,
          updated_at = NOW()
        "#,
    )
    .bind(auth.user_id)
    .bind(id)
    .bind(calendar_id)
    .bind(&google_event.id)
    .bind(&google_event.html_link)
    .bind(fingerprint)
    .execute(&state.pool)
    .await?;
    Ok(Json(AddEventResponse {
        google_event_id: google_event.id,
        html_link: google_event.html_link,
        created,
    }))
}

async fn insert_google_event(
    state: &AppState,
    token: &str,
    calendar_id: &str,
    body: &serde_json::Value,
) -> Result<GoogleEventResponse, AppError> {
    let response = state
        .provider_http_client
        .post(google_event_url(calendar_id, None)?)
        .bearer_auth(token)
        .json(body)
        .send()
        .await
        .map_err(|error| AppError::BadGateway(format!("Google Calendar insert failed: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::BadGateway(format!(
            "Google Calendar insert failed with status {}",
            response.status()
        )));
    }
    response
        .json::<GoogleEventResponse>()
        .await
        .map_err(|error| AppError::BadGateway(format!("Google returned an invalid event: {error}")))
}

fn google_event_url(calendar_id: &str, event_id: Option<&str>) -> Result<Url, AppError> {
    let mut url = Url::parse("https://www.googleapis.com/calendar/v3/calendars/")
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| AppError::Internal(anyhow!("invalid Google Calendar API URL")))?;
        segments.pop_if_empty().push(calendar_id).push("events");
        if let Some(value) = event_id {
            segments.push(value);
        }
    }
    Ok(url)
}

fn calendar_event_body(
    state: &AppState,
    event: &sports::SportsEventResponse,
) -> Result<serde_json::Value, AppError> {
    let start = DateTime::parse_from_rfc3339(&event.start_time)
        .map_err(|_| {
            AppError::BadGateway("Sports API returned an invalid event start time".to_string())
        })?
        .with_timezone(&Utc);
    let end = match event.end_time.as_deref() {
        Some(value) => DateTime::parse_from_rfc3339(value)
            .map_err(|_| {
                AppError::BadGateway("Sports API returned an invalid event end time".to_string())
            })?
            .with_timezone(&Utc),
        None => start + ChronoDuration::hours(2),
    };
    let participants = event
        .participants
        .as_ref()
        .map(|value| match (&value.home, &value.away) {
            (Some(home), Some(away)) => format!("{home} vs {away}"),
            _ => event.title.clone(),
        })
        .unwrap_or_else(|| event.title.clone());
    let mut description = vec![format!("Competition: {}", event.competition)];
    if let Some(round) = &event.round_label {
        description.push(format!("Round: {round}"));
    }
    if let Some(provider) = &event.watch.recommended_provider {
        description.push(format!(
            "Recommended: {provider}{}",
            event
                .watch
                .recommended_market
                .as_ref()
                .map(|market| format!(" ({})", market.to_uppercase()))
                .unwrap_or_default()
        ));
    }
    description.push(String::new());
    description.push("Watch options:".to_string());
    if event.watch.availabilities.is_empty() {
        description.push("• Watch guidance is not available yet.".to_string());
    } else {
        for option in &event.watch.availabilities {
            let channel = option
                .channel_name
                .as_ref()
                .map(|value| format!(" — {value}"))
                .unwrap_or_default();
            let mut details = Vec::new();
            if let Some(market) = &option.market {
                details.push(market.to_uppercase());
            }
            if let Some(kind) = &option.watch_type {
                details.push(kind.clone());
            }
            let suffix = if details.is_empty() {
                String::new()
            } else {
                format!(" ({})", details.join(", "))
            };
            description.push(format!("• {}{channel}{suffix}", option.provider_label));
        }
    }
    if let Some(origin) = &state.config.public_origin {
        let mut link = origin
            .join("/sports")
            .map_err(|error| AppError::Internal(anyhow!(error)))?;
        link.query_pairs_mut().append_pair("event", &event.id);
        description.push(String::new());
        description.push(format!("Open in Euripus: {link}"));
    }
    if let Some(source) = &event.source_url {
        description.push(format!("Source: {source}"));
    }
    Ok(serde_json::json!({
        "summary": format!("{participants} · {}", event.competition),
        "description": description.join("\n"),
        "location": event.venue,
        "start": { "dateTime": start.to_rfc3339() },
        "end": { "dateTime": end.to_rfc3339() },
        "extendedProperties": { "private": { "euripusSportsEventId": event.id } }
    }))
}

fn random_urlsafe(length: usize) -> String {
    let mut bytes = vec![0u8; length];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_url_encodes_identifiers_as_path_segments() {
        let url = google_event_url("calendar/user@example.com", Some("event id")).unwrap();
        assert!(
            url.as_str()
                .contains("calendar%2Fuser@example.com/events/event%20id")
        );
    }
}
