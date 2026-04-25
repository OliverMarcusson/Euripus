use super::playback::resolve::PlaybackSourceResponse;
use super::playback::{
    resolve_channel_playback_source_for_receiver, resolve_program_playback_source_for_receiver,
};
use super::*;

pub(super) fn browser_router() -> Router<AppState> {
    Router::new()
        .route("/receiver/session", post(create_receiver_session))
        .route("/receiver/pairing-code", post(issue_receiver_pairing_code))
        .route("/receiver/events", get(stream_receiver_events))
        .route("/receiver/heartbeat", post(heartbeat_receiver))
        .route(
            "/receiver/playback-state",
            post(update_receiver_playback_state),
        )
        .route(
            "/receiver/commands/{command_id}/ack",
            post(acknowledge_receiver_command),
        )
}

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/receiver/pair", post(pair_receiver))
        .route("/remote/receivers", get(list_remote_receivers))
        .route("/remote/pair", post(pair_receiver))
        .route("/remote/receivers/{id}", delete(unpair_receiver))
        .route(
            "/remote/controller/target",
            get(get_remote_controller_target)
                .post(select_remote_controller_target)
                .delete(clear_remote_controller_target),
        )
        .route("/remote/play/channel/{id}", post(play_channel_remotely))
        .route("/remote/play/program/{id}", post(play_program_remotely))
        .route("/remote/command/pause", post(pause_remote_playback))
        .route("/remote/command/play", post(resume_remote_playback))
        .route("/remote/command/seek", post(seek_remote_playback))
        .route("/remote/command/stop", post(stop_remote_playback))
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverPlaybackStateResponse {
    pub(super) title: String,
    pub(super) source_kind: String,
    pub(super) live: bool,
    pub(super) catchup: bool,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) paused: bool,
    pub(super) buffering: bool,
    pub(super) position_seconds: Option<f64>,
    pub(super) duration_seconds: Option<f64>,
    pub(super) error_message: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverDeviceResponse {
    pub(super) id: Uuid,
    pub(super) name: String,
    pub(super) platform: String,
    pub(super) form_factor_hint: Option<String>,
    pub(super) app_kind: String,
    pub(super) remembered: bool,
    pub(super) online: bool,
    pub(super) current_controller: bool,
    pub(super) last_seen_at: DateTime<Utc>,
    pub(super) updated_at: DateTime<Utc>,
    pub(super) current_playback: Option<ReceiverPlaybackStateResponse>,
    pub(super) playback_state_stale: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteControllerTargetResponse {
    pub(super) device: ReceiverDeviceResponse,
    pub(super) selected_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone, FromRow)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemotePlaybackCommandResponse {
    pub(super) id: Uuid,
    pub(super) target_device_id: Uuid,
    pub(super) target_device_name: String,
    pub(super) command_type: String,
    pub(super) status: String,
    pub(super) source_title: String,
    pub(super) created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverEventPayload {
    pub(super) event_type: String,
    pub(super) command: RemotePlaybackCommandResponse,
    pub(super) source: Option<PlaybackSourceResponse>,
    pub(super) position_seconds: Option<f64>,
    pub(super) receiver_credential: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteControllerTargetPayload {
    pub(super) device_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverSessionPayload {
    pub(super) device_key: String,
    pub(super) name: String,
    pub(super) platform: String,
    pub(super) form_factor_hint: Option<String>,
    pub(super) app_kind: String,
    pub(super) public_origin: Option<String>,
    pub(super) receiver_credential: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverSessionResponse {
    pub(super) session_token: String,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) receiver_credential: Option<String>,
    pub(super) device: ReceiverDeviceResponse,
    pub(super) pairing_code: Option<String>,
    pub(super) paired: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PairReceiverPayload {
    pub(super) code: String,
    pub(super) remember_device: bool,
    pub(super) name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PairingCodeResponse {
    pub(super) code: String,
    pub(super) expires_at: DateTime<Utc>,
    pub(super) device: ReceiverDeviceResponse,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverPlaybackStatePayload {
    pub(super) title: Option<String>,
    pub(super) source_kind: Option<String>,
    pub(super) live: Option<bool>,
    pub(super) catchup: Option<bool>,
    pub(super) paused: Option<bool>,
    pub(super) buffering: Option<bool>,
    pub(super) position_seconds: Option<f64>,
    pub(super) duration_seconds: Option<f64>,
    pub(super) error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverTransportPayload {
    pub(super) position_seconds: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RemoteCommandAckPayload {
    pub(super) status: String,
    pub(super) error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReceiverEventsQuery {
    pub(super) session_token: Option<String>,
}

#[derive(Debug, FromRow, Clone)]
pub(super) struct ReceiverDeviceRecord {
    pub(super) id: Uuid,
    pub(super) owner_user_id: Option<Uuid>,
    pub(super) device_name: String,
    pub(super) platform: String,
    pub(super) form_factor_hint: Option<String>,
    pub(super) app_kind: String,
    pub(super) remembered: bool,
    pub(super) last_seen_at: DateTime<Utc>,
    pub(super) current_playback_title: Option<String>,
    pub(super) current_playback_kind: Option<String>,
    pub(super) current_playback_live: Option<bool>,
    pub(super) current_playback_catchup: Option<bool>,
    pub(super) current_playback_updated_at: Option<DateTime<Utc>>,
    pub(super) current_playback_paused: Option<bool>,
    pub(super) current_playback_buffering: Option<bool>,
    pub(super) current_playback_position_seconds: Option<f64>,
    pub(super) current_playback_duration_seconds: Option<f64>,
    pub(super) current_playback_error_message: Option<String>,
    pub(super) last_public_origin: Option<String>,
    pub(super) revoked_at: Option<DateTime<Utc>>,
    pub(super) updated_at: DateTime<Utc>,
}

#[derive(Debug, FromRow)]
pub(super) struct ReceiverSessionRecord {
    pub(super) id: Uuid,
    pub(super) receiver_device_id: Uuid,
}

#[derive(Debug, FromRow)]
pub(super) struct ReceiverPairingCodeRecord {
    pub(super) id: Uuid,
    pub(super) receiver_device_id: Uuid,
    pub(super) code: String,
    pub(super) expires_at: DateTime<Utc>,
}

#[derive(Debug, FromRow, Clone)]
pub(super) struct ReceiverControllerTargetRecord {
    pub(super) selected_at: DateTime<Utc>,
    pub(super) id: Uuid,
    pub(super) owner_user_id: Option<Uuid>,
    pub(super) device_name: String,
    pub(super) platform: String,
    pub(super) form_factor_hint: Option<String>,
    pub(super) app_kind: String,
    pub(super) remembered: bool,
    pub(super) last_seen_at: DateTime<Utc>,
    pub(super) current_playback_title: Option<String>,
    pub(super) current_playback_kind: Option<String>,
    pub(super) current_playback_live: Option<bool>,
    pub(super) current_playback_catchup: Option<bool>,
    pub(super) current_playback_updated_at: Option<DateTime<Utc>>,
    pub(super) current_playback_paused: Option<bool>,
    pub(super) current_playback_buffering: Option<bool>,
    pub(super) current_playback_position_seconds: Option<f64>,
    pub(super) current_playback_duration_seconds: Option<f64>,
    pub(super) current_playback_error_message: Option<String>,
    pub(super) last_public_origin: Option<String>,
    pub(super) revoked_at: Option<DateTime<Utc>>,
    pub(super) updated_at: DateTime<Utc>,
}

async fn create_receiver_session(
    State(state): State<AppState>,
    Json(payload): Json<ReceiverSessionPayload>,
) -> ApiResult<ReceiverSessionResponse> {
    let device_key = payload.device_key.trim();
    let device_name = payload.name.trim();
    let platform = payload.platform.trim();
    let app_kind = payload.app_kind.trim();
    let provided_receiver_credential = payload
        .receiver_credential
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if device_key.is_empty() || device_name.is_empty() || platform.is_empty() || app_kind.is_empty()
    {
        return Err(AppError::BadRequest(
            "Device key, name, platform, and app kind are required.".to_string(),
        ));
    }

    let now = Utc::now();
    let existing = if let Some(receiver_credential) = provided_receiver_credential {
        let hash = hash_receiver_token(receiver_credential);
        sqlx::query_as::<_, ReceiverDeviceRecord>(
            r#"
            SELECT id, owner_user_id, device_name, platform, form_factor_hint, app_kind,
                   remembered, last_seen_at,
                   current_playback_title, current_playback_kind, current_playback_live,
                   current_playback_catchup, current_playback_updated_at, current_playback_paused,
                   current_playback_buffering, current_playback_position_seconds,
                   current_playback_duration_seconds, current_playback_error_message,
                   last_public_origin,
                   revoked_at, updated_at
            FROM receiver_devices
            WHERE receiver_credential_hash = $1 AND revoked_at IS NULL
            "#,
        )
        .bind(hash)
        .fetch_optional(&state.pool)
        .await?
    } else {
        None
    };
    let authenticated_with_receiver_credential = existing.is_some();

    let record = if let Some(existing) = existing {
        sqlx::query_as::<_, ReceiverDeviceRecord>(
            r#"
            UPDATE receiver_devices
            SET device_key = $2,
                device_name = CASE
                    WHEN owner_user_id IS NULL THEN $3
                    ELSE device_name
                END,
                platform = $4, form_factor_hint = $5,
                app_kind = $6, last_public_origin = $7, last_seen_at = NOW(), updated_at = NOW()
            WHERE id = $1
            RETURNING id, owner_user_id, device_name, platform, form_factor_hint, app_kind,
                   remembered, last_seen_at,
                   current_playback_title, current_playback_kind, current_playback_live,
                   current_playback_catchup, current_playback_updated_at, current_playback_paused,
                   current_playback_buffering, current_playback_position_seconds,
                   current_playback_duration_seconds, current_playback_error_message,
                   last_public_origin,
                   revoked_at, updated_at
            "#,
        )
        .bind(existing.id)
        .bind(device_key)
        .bind(device_name)
        .bind(platform)
        .bind(payload.form_factor_hint)
        .bind(app_kind)
        .bind(payload.public_origin.as_deref())
        .fetch_one(&state.pool)
        .await?
    } else {
        sqlx::query_as::<_, ReceiverDeviceRecord>(
            r#"
            INSERT INTO receiver_devices (
                device_key, device_name, platform, form_factor_hint, app_kind, last_public_origin, last_seen_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, NOW(), NOW())
            ON CONFLICT (device_key)
            DO UPDATE SET device_name = CASE
                              WHEN receiver_devices.owner_user_id IS NULL THEN EXCLUDED.device_name
                              ELSE receiver_devices.device_name
                          END,
                          platform = EXCLUDED.platform,
                          form_factor_hint = EXCLUDED.form_factor_hint,
                          app_kind = EXCLUDED.app_kind,
                          last_public_origin = EXCLUDED.last_public_origin,
                          last_seen_at = NOW(),
                          updated_at = NOW()
            RETURNING id, owner_user_id, device_name, platform, form_factor_hint, app_kind,
                   remembered, last_seen_at,
                   current_playback_title, current_playback_kind, current_playback_live,
                   current_playback_catchup, current_playback_updated_at, current_playback_paused,
                   current_playback_buffering, current_playback_position_seconds,
                   current_playback_duration_seconds, current_playback_error_message,
                   last_public_origin,
                   revoked_at, updated_at
            "#,
        )
        .bind(device_key)
        .bind(device_name)
        .bind(platform)
        .bind(payload.form_factor_hint)
        .bind(app_kind)
        .bind(payload.public_origin.as_deref())
        .fetch_one(&state.pool)
        .await?
    };

    sqlx::query(
        "UPDATE receiver_sessions SET closed_at = NOW(), updated_at = NOW() WHERE receiver_device_id = $1 AND closed_at IS NULL",
    )
    .bind(record.id)
    .execute(&state.pool)
    .await?;

    let session_token = generate_refresh_token();
    let expires_at = now + ChronoDuration::hours(RECEIVER_SESSION_TTL_HOURS);
    sqlx::query(
        r#"
        INSERT INTO receiver_sessions (receiver_device_id, session_token_hash, expires_at, last_seen_at, created_at, updated_at)
        VALUES ($1, $2, $3, NOW(), NOW(), NOW())
        "#,
    )
    .bind(record.id)
    .bind(hash_receiver_token(&session_token))
    .bind(expires_at)
        .execute(&state.pool)
        .await?;

    let session_receiver_credential = if record.owner_user_id.is_some() && record.remembered {
        if authenticated_with_receiver_credential {
            provided_receiver_credential.map(str::to_owned)
        } else {
            let receiver_credential = generate_refresh_token();
            sqlx::query(
                r#"
                UPDATE receiver_devices
                SET receiver_credential_hash = $2,
                    updated_at = NOW()
                WHERE id = $1
                "#,
            )
            .bind(record.id)
            .bind(hash_receiver_token(&receiver_credential))
            .execute(&state.pool)
            .await?;
            Some(receiver_credential)
        }
    } else {
        None
    };

    let pairing_code = if record.owner_user_id.is_none() {
        Some(refresh_pairing_code(&state.pool, record.id).await?)
    } else {
        None
    };

    Ok(Json(ReceiverSessionResponse {
        session_token,
        expires_at,
        receiver_credential: session_receiver_credential,
        device: receiver_device_response(&state, &record, None),
        pairing_code: pairing_code.as_ref().map(|value| value.code.clone()),
        paired: record.owner_user_id.is_some() && record.revoked_at.is_none(),
    }))
}

async fn issue_receiver_pairing_code(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<PairingCodeResponse> {
    let receiver = require_receiver_auth(&state, &headers).await?;
    let record = load_receiver_device(&state.pool, receiver.receiver_device_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Receiver not found".to_string()))?;
    if record.owner_user_id.is_some() && record.remembered {
        return Err(AppError::BadRequest(
            "This receiver is already paired.".to_string(),
        ));
    }
    let pairing = refresh_pairing_code(&state.pool, record.id).await?;
    Ok(Json(PairingCodeResponse {
        code: pairing.code,
        expires_at: pairing.expires_at,
        device: receiver_device_response(&state, &record, None),
    }))
}

async fn heartbeat_receiver(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let receiver = require_receiver_auth(&state, &headers).await?;
    sqlx::query(
        r#"
        UPDATE receiver_sessions
        SET last_seen_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND receiver_device_id = $2
        "#,
    )
    .bind(receiver.receiver_session_id)
    .bind(receiver.receiver_device_id)
    .execute(&state.pool)
    .await?;
    sqlx::query(
        "UPDATE receiver_devices SET last_seen_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(receiver.receiver_device_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn stream_receiver_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ReceiverEventsQuery>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, std::convert::Infallible>>>, AppError>
{
    let receiver =
        require_receiver_auth_with_optional_query_token(&state, &headers, query.session_token)
            .await?;
    let device = load_receiver_device(&state.pool, receiver.receiver_device_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Receiver not found".to_string()))?;
    let sender = receiver_sender(&state, receiver.receiver_device_id);
    let initial_events = if device.owner_user_id.is_some() && device.revoked_at.is_none() {
        vec![receiver_event_to_sse(ReceiverEventPayload {
            event_type: "pairing_complete".to_string(),
            command: RemotePlaybackCommandResponse {
                id: Uuid::new_v4(),
                target_device_id: device.id,
                target_device_name: device.device_name.clone(),
                command_type: "pairing".to_string(),
                status: "delivered".to_string(),
                source_title: device.device_name.clone(),
                created_at: Utc::now(),
            },
            source: None,
            position_seconds: None,
            receiver_credential: None,
        })]
    } else {
        Vec::new()
    };
    let live_events =
        BroadcastStream::new(sender.subscribe()).filter_map(|message| match message {
            Ok(payload) => Some(receiver_event_to_sse(payload)),
            Err(_) => None,
        });
    let cleanup_guard = ReceiverChannelCleanupGuard {
        channels: state.receiver_channels.clone(),
        device_id: receiver.receiver_device_id,
        sender: sender.clone(),
    };
    let stream = futures_util::stream::iter(initial_events)
        .chain(live_events)
        .map(move |event| {
            let _cleanup_guard = &cleanup_guard;
            event
        });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn update_receiver_playback_state(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReceiverPlaybackStatePayload>,
) -> Result<StatusCode, AppError> {
    let receiver = require_receiver_auth(&state, &headers).await?;
    sqlx::query(
        r#"
        UPDATE receiver_devices
        SET current_playback_title = $2,
            current_playback_kind = $3,
            current_playback_live = $4,
            current_playback_catchup = $5,
            current_playback_updated_at = CASE WHEN $2 IS NULL THEN NULL ELSE NOW() END,
            current_playback_paused = $6,
            current_playback_buffering = $7,
            current_playback_position_seconds = $8,
            current_playback_duration_seconds = $9,
            current_playback_error_message = $10,
            last_seen_at = NOW(),
            updated_at = NOW()
        WHERE id = $1
        "#,
    )
    .bind(receiver.receiver_device_id)
    .bind(payload.title)
    .bind(payload.source_kind)
    .bind(payload.live)
    .bind(payload.catchup)
    .bind(payload.paused)
    .bind(payload.buffering)
    .bind(payload.position_seconds)
    .bind(payload.duration_seconds)
    .bind(payload.error_message)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn acknowledge_receiver_command(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(command_id): Path<Uuid>,
    Json(payload): Json<RemoteCommandAckPayload>,
) -> Result<StatusCode, AppError> {
    let receiver = require_receiver_auth(&state, &headers).await?;
    let normalized_status = normalize_command_status(&payload.status)?;
    let updated = sqlx::query(
        r#"
        UPDATE receiver_commands
        SET status = $2,
            error_message = $3,
            delivered_at = CASE WHEN $2 = 'delivered' THEN NOW() ELSE delivered_at END,
            executing_at = CASE WHEN $2 = 'executing' THEN NOW() ELSE executing_at END,
            acknowledged_at = CASE WHEN $2 = 'succeeded' THEN NOW() ELSE acknowledged_at END,
            completed_at = CASE WHEN $2 IN ('succeeded', 'failed') THEN NOW() ELSE completed_at END,
            failed_at = CASE WHEN $2 = 'failed' THEN NOW() ELSE failed_at END
        WHERE id = $1 AND receiver_device_id = $4
        "#,
    )
    .bind(command_id)
    .bind(normalized_status)
    .bind(payload.error_message)
    .bind(receiver.receiver_device_id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(AppError::NotFound("Receiver command not found".to_string()));
    }
    Ok(StatusCode::NO_CONTENT)
}

fn normalize_command_status(status: &str) -> Result<&'static str, AppError> {
    match status {
        "delivered" => Ok("delivered"),
        "executing" => Ok("executing"),
        "succeeded" | "acknowledged" => Ok("succeeded"),
        "failed" => Ok("failed"),
        _ => Err(AppError::BadRequest(
            "Unsupported receiver command status.".to_string(),
        )),
    }
}

async fn pair_receiver(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PairReceiverPayload>,
) -> ApiResult<ReceiverDeviceResponse> {
    let auth = require_auth(&state, &headers).await?;
    let code = payload.code.trim().to_uppercase();
    let pairing = sqlx::query_as::<_, ReceiverPairingCodeRecord>(
        r#"
        SELECT id, receiver_device_id, code, expires_at
        FROM receiver_pairing_codes
        WHERE code = $1 AND claimed_at IS NULL AND expires_at > NOW()
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(&code)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::BadRequest("That pairing code is not valid.".to_string()))?;

    let receiver_credential = payload.remember_device.then(generate_refresh_token);
    let credential_hash = receiver_credential
        .as_ref()
        .map(|value| hash_receiver_token(value));
    let name = payload
        .name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let record = sqlx::query_as::<_, ReceiverDeviceRecord>(
        r#"
        UPDATE receiver_devices
        SET owner_user_id = $2,
            device_name = COALESCE($3, device_name),
            remembered = $4,
            receiver_credential_hash = $5,
            paired_at = NOW(),
            revoked_at = NULL,
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, owner_user_id, device_name, platform, form_factor_hint, app_kind,
               remembered, last_seen_at,
               current_playback_title, current_playback_kind, current_playback_live,
               current_playback_catchup, current_playback_updated_at, current_playback_paused,
               current_playback_buffering, current_playback_position_seconds,
               current_playback_duration_seconds, current_playback_error_message,
               last_public_origin,
               revoked_at, updated_at
        "#,
    )
    .bind(pairing.receiver_device_id)
    .bind(auth.user_id)
    .bind(name)
    .bind(payload.remember_device)
    .bind(credential_hash)
    .fetch_one(&state.pool)
    .await?;

    sqlx::query("UPDATE receiver_pairing_codes SET claimed_at = NOW() WHERE id = $1")
        .bind(pairing.id)
        .execute(&state.pool)
        .await?;

    let _ = receiver_sender(&state, record.id).send(ReceiverEventPayload {
        event_type: "pairing_complete".to_string(),
        command: RemotePlaybackCommandResponse {
            id: Uuid::new_v4(),
            target_device_id: record.id,
            target_device_name: record.device_name.clone(),
            command_type: "pairing".to_string(),
            status: "delivered".to_string(),
            source_title: record.device_name.clone(),
            created_at: Utc::now(),
        },
        source: None,
        position_seconds: None,
        receiver_credential,
    });

    let response = receiver_device_response(&state, &record, None);
    Ok(Json(response))
}

async fn list_remote_receivers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Vec<ReceiverDeviceResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let records = sqlx::query_as::<_, ReceiverDeviceRecord>(
        r#"
        SELECT id, owner_user_id, device_name, platform, form_factor_hint, app_kind,
               remembered, last_seen_at,
               current_playback_title, current_playback_kind, current_playback_live,
               current_playback_catchup, current_playback_updated_at, current_playback_paused,
               current_playback_buffering, current_playback_position_seconds,
               current_playback_duration_seconds, current_playback_error_message,
               last_public_origin,
               revoked_at, updated_at
        FROM receiver_devices
        WHERE owner_user_id = $1 AND revoked_at IS NULL
        ORDER BY updated_at DESC
        "#,
    )
    .bind(auth.user_id)
    .fetch_all(&state.pool)
    .await?;
    let current_controller_device_id =
        load_receiver_controller_target_record(&state.pool, auth.user_id, auth.session_id)
            .await?
            .map(|record| record.id);

    let items = records
        .into_iter()
        .filter(|record| record.remembered || is_receiver_online(&state, record))
        .map(|record| receiver_device_response(&state, &record, current_controller_device_id))
        .collect();
    Ok(Json(items))
}

async fn unpair_receiver(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    let updated = sqlx::query(
        r#"
        UPDATE receiver_devices
        SET owner_user_id = NULL, remembered = FALSE, receiver_credential_hash = NULL,
            paired_at = NULL, revoked_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND owner_user_id = $2
        "#,
    )
    .bind(id)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if updated == 0 {
        return Err(AppError::NotFound("Receiver not found".to_string()));
    }
    sqlx::query("DELETE FROM receiver_controller_sessions WHERE receiver_device_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if let Some((_, sender)) = state.receiver_channels.remove(&id) {
        let _ = sender.send(receiver_terminal_event(id));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn get_remote_controller_target(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Option<RemoteControllerTargetResponse>> {
    let auth = require_auth(&state, &headers).await?;
    let target =
        load_receiver_controller_target_record(&state.pool, auth.user_id, auth.session_id).await?;

    let Some(record) = target else {
        return Ok(Json(None));
    };
    let device_record = receiver_device_record_from_target(record.clone());
    if !is_receiver_online(&state, &device_record) {
        return Ok(Json(None));
    }

    Ok(Json(Some(RemoteControllerTargetResponse {
        device: receiver_device_response(&state, &device_record, Some(record.id)),
        selected_at: record.selected_at,
    })))
}

async fn select_remote_controller_target(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RemoteControllerTargetPayload>,
) -> ApiResult<RemoteControllerTargetResponse> {
    let auth = require_auth(&state, &headers).await?;
    let device = load_receiver_device(&state.pool, payload.device_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Receiver not found".to_string()))?;

    if device.owner_user_id != Some(auth.user_id) || !is_receiver_online(&state, &device) {
        return Err(AppError::BadRequest(
            "That receiver is not currently available.".to_string(),
        ));
    }

    sqlx::query(
        r#"
        INSERT INTO receiver_controller_sessions (
          controller_session_id,
          user_id,
          receiver_device_id,
          created_at,
          updated_at
        )
        VALUES ($1, $2, $3, NOW(), NOW())
        ON CONFLICT (controller_session_id)
        DO UPDATE SET receiver_device_id = EXCLUDED.receiver_device_id, updated_at = NOW()
        "#,
    )
    .bind(auth.session_id)
    .bind(auth.user_id)
    .bind(device.id)
    .execute(&state.pool)
    .await?;

    let selected =
        load_receiver_controller_target_record(&state.pool, auth.user_id, auth.session_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Remote controller target not found".to_string()))?;

    Ok(Json(RemoteControllerTargetResponse {
        device: receiver_device_response(
            &state,
            &receiver_device_record_from_target(selected.clone()),
            Some(selected.id),
        ),
        selected_at: selected.selected_at,
    }))
}

async fn clear_remote_controller_target(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let auth = require_auth(&state, &headers).await?;
    sqlx::query(
        "DELETE FROM receiver_controller_sessions WHERE controller_session_id = $1 AND user_id = $2",
    )
    .bind(auth.session_id)
    .bind(auth.user_id)
    .execute(&state.pool)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn play_channel_remotely(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    let source = resolve_channel_playback_source_for_receiver(
        &state,
        &headers,
        auth.user_id,
        id,
        &target.app_kind,
        target.last_public_origin.as_deref(),
    )
    .await?;

    Ok(Json(
        deliver_remote_playback_command(&state, &auth, &target, source).await?,
    ))
}

async fn play_program_remotely(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    let source = resolve_program_playback_source_for_receiver(
        &state,
        &headers,
        auth.user_id,
        id,
        &target.app_kind,
        target.last_public_origin.as_deref(),
    )
    .await?;

    Ok(Json(
        deliver_remote_playback_command(&state, &auth, &target, source).await?,
    ))
}

async fn pause_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(&state, &auth, &target, "pause", None).await?,
    ))
}

async fn resume_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(&state, &auth, &target, "play", None).await?,
    ))
}

async fn seek_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReceiverTransportPayload>,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(
            &state,
            &auth,
            &target,
            "seek",
            payload.position_seconds,
        )
        .await?,
    ))
}

async fn stop_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(&state, &auth, &target, "stop", None).await?,
    ))
}

fn is_receiver_online(state: &AppState, device: &ReceiverDeviceRecord) -> bool {
    if device.revoked_at.is_some() {
        return false;
    }

    let ttl = ChronoDuration::from_std(RECEIVER_TTL).expect("receiver ttl");
    let fresh = device.last_seen_at + ttl > Utc::now();
    let connected = state
        .receiver_channels
        .get(&device.id)
        .map(|sender| sender.value().receiver_count() > 0)
        .unwrap_or(false);

    fresh && connected
}

fn receiver_sender(state: &AppState, device_id: Uuid) -> broadcast::Sender<ReceiverEventPayload> {
    state
        .receiver_channels
        .entry(device_id)
        .or_insert_with(|| broadcast::channel(32).0)
        .clone()
}

struct ReceiverChannelCleanupGuard {
    channels: Arc<DashMap<Uuid, broadcast::Sender<ReceiverEventPayload>>>,
    device_id: Uuid,
    sender: broadcast::Sender<ReceiverEventPayload>,
}

impl Drop for ReceiverChannelCleanupGuard {
    fn drop(&mut self) {
        // Depending on stream drop order, this subscriber may still be counted here.
        let should_remove = self
            .channels
            .get(&self.device_id)
            .map(|entry| entry.same_channel(&self.sender) && entry.receiver_count() <= 1)
            .unwrap_or(false);

        if should_remove {
            self.channels.remove_if(&self.device_id, |_, sender| {
                sender.same_channel(&self.sender) && sender.receiver_count() <= 1
            });
        }
    }
}

fn receiver_terminal_event(device_id: Uuid) -> ReceiverEventPayload {
    ReceiverEventPayload {
        event_type: "receiver_revoked".to_string(),
        command: RemotePlaybackCommandResponse {
            id: Uuid::new_v4(),
            target_device_id: device_id,
            target_device_name: "Receiver".to_string(),
            command_type: "receiver_revoked".to_string(),
            status: "delivered".to_string(),
            source_title: "Receiver unpaired".to_string(),
            created_at: Utc::now(),
        },
        source: None,
        position_seconds: None,
        receiver_credential: None,
    }
}

fn receiver_event_to_sse(payload: ReceiverEventPayload) -> Result<Event, std::convert::Infallible> {
    Ok(Event::default()
        .event(payload.event_type.clone())
        .json_data(payload)
        .expect("receiver event payload should serialize"))
}

pub(super) async fn load_receiver_device(
    pool: &PgPool,
    device_id: Uuid,
) -> Result<Option<ReceiverDeviceRecord>, AppError> {
    sqlx::query_as::<_, ReceiverDeviceRecord>(
        r#"
        SELECT id, owner_user_id, device_name, platform, form_factor_hint, app_kind,
               remembered, last_seen_at,
               current_playback_title, current_playback_kind, current_playback_live,
               current_playback_catchup, current_playback_updated_at, current_playback_paused,
               current_playback_buffering, current_playback_position_seconds,
               current_playback_duration_seconds, current_playback_error_message,
               last_public_origin,
               revoked_at, updated_at
        FROM receiver_devices
        WHERE id = $1
        "#,
    )
    .bind(device_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

async fn load_receiver_controller_target_record(
    pool: &PgPool,
    user_id: Uuid,
    controller_session_id: Uuid,
) -> Result<Option<ReceiverControllerTargetRecord>, AppError> {
    sqlx::query_as::<_, ReceiverControllerTargetRecord>(
        r#"
        SELECT
          rcs.updated_at AS selected_at,
          rd.id,
          rd.owner_user_id,
          rd.device_name,
          rd.platform,
          rd.form_factor_hint,
          rd.app_kind,
          rd.remembered,
          rd.last_seen_at,
          rd.current_playback_title,
          rd.current_playback_kind,
          rd.current_playback_live,
          rd.current_playback_catchup,
          rd.current_playback_updated_at,
          rd.current_playback_paused,
          rd.current_playback_buffering,
          rd.current_playback_position_seconds,
          rd.current_playback_duration_seconds,
          rd.current_playback_error_message,
          rd.last_public_origin,
          rd.revoked_at,
          rd.updated_at
        FROM receiver_controller_sessions rcs
        JOIN receiver_devices rd ON rd.id = rcs.receiver_device_id
        WHERE rcs.controller_session_id = $1 AND rcs.user_id = $2
        "#,
    )
    .bind(controller_session_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::from)
}

fn receiver_device_record_from_target(
    record: ReceiverControllerTargetRecord,
) -> ReceiverDeviceRecord {
    ReceiverDeviceRecord {
        id: record.id,
        owner_user_id: record.owner_user_id,
        device_name: record.device_name,
        platform: record.platform,
        form_factor_hint: record.form_factor_hint,
        app_kind: record.app_kind,
        remembered: record.remembered,
        last_seen_at: record.last_seen_at,
        current_playback_title: record.current_playback_title,
        current_playback_kind: record.current_playback_kind,
        current_playback_live: record.current_playback_live,
        current_playback_catchup: record.current_playback_catchup,
        current_playback_updated_at: record.current_playback_updated_at,
        current_playback_paused: record.current_playback_paused,
        current_playback_buffering: record.current_playback_buffering,
        current_playback_position_seconds: record.current_playback_position_seconds,
        current_playback_duration_seconds: record.current_playback_duration_seconds,
        current_playback_error_message: record.current_playback_error_message,
        last_public_origin: record.last_public_origin,
        revoked_at: record.revoked_at,
        updated_at: record.updated_at,
    }
}

fn receiver_device_response(
    state: &AppState,
    record: &ReceiverDeviceRecord,
    current_controller_device_id: Option<Uuid>,
) -> ReceiverDeviceResponse {
    let online = is_receiver_online(state, record);
    let playback_state_stale = record.current_playback_title.is_some() && !online;

    ReceiverDeviceResponse {
        id: record.id,
        name: record.device_name.clone(),
        platform: record.platform.clone(),
        form_factor_hint: record.form_factor_hint.clone(),
        app_kind: record.app_kind.clone(),
        remembered: record.remembered,
        online,
        current_controller: online && current_controller_device_id == Some(record.id),
        last_seen_at: record.last_seen_at,
        updated_at: record.updated_at,
        current_playback: (!playback_state_stale)
            .then(|| playback_state_from_record(record))
            .flatten(),
        playback_state_stale,
    }
}

fn playback_state_from_record(
    record: &ReceiverDeviceRecord,
) -> Option<ReceiverPlaybackStateResponse> {
    record
        .current_playback_title
        .as_ref()
        .map(|title| ReceiverPlaybackStateResponse {
            title: title.clone(),
            source_kind: record.current_playback_kind.clone().unwrap_or_default(),
            live: record.current_playback_live.unwrap_or(false),
            catchup: record.current_playback_catchup.unwrap_or(false),
            updated_at: record
                .current_playback_updated_at
                .unwrap_or(record.updated_at),
            paused: record.current_playback_paused.unwrap_or(false),
            buffering: record.current_playback_buffering.unwrap_or(false),
            position_seconds: record.current_playback_position_seconds,
            duration_seconds: record.current_playback_duration_seconds,
            error_message: record.current_playback_error_message.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_command_status_accepts_legacy_acknowledged() {
        assert_eq!(
            normalize_command_status("acknowledged").unwrap(),
            "succeeded"
        );
        assert_eq!(normalize_command_status("executing").unwrap(), "executing");
        assert!(normalize_command_status("bogus").is_err());
    }

    #[test]
    fn playback_state_from_record_includes_buffering_and_error() {
        let now = Utc::now();
        let record = ReceiverDeviceRecord {
            id: Uuid::new_v4(),
            owner_user_id: None,
            device_name: "TV".to_string(),
            platform: "android-tv".to_string(),
            form_factor_hint: Some("tv".to_string()),
            app_kind: "receiver-android-tv".to_string(),
            remembered: true,
            last_seen_at: now,
            current_playback_title: Some("Arena 1".to_string()),
            current_playback_kind: Some("hls".to_string()),
            current_playback_live: Some(true),
            current_playback_catchup: Some(false),
            current_playback_updated_at: Some(now),
            current_playback_paused: Some(false),
            current_playback_buffering: Some(true),
            current_playback_position_seconds: Some(12.0),
            current_playback_duration_seconds: None,
            current_playback_error_message: Some(
                "The receiver could not decode this stream.".to_string(),
            ),
            last_public_origin: Some("http://192.168.0.67:5173".to_string()),
            revoked_at: None,
            updated_at: now,
        };

        let playback = playback_state_from_record(&record).expect("playback state");
        assert!(playback.buffering);
        assert_eq!(
            playback.error_message.as_deref(),
            Some("The receiver could not decode this stream."),
        );
    }
}

async fn current_remote_target_for_control(
    state: &AppState,
    auth: &AuthContext,
) -> Result<ReceiverDeviceRecord, AppError> {
    let target = load_receiver_controller_target_record(&state.pool, auth.user_id, auth.session_id)
        .await?
        .map(receiver_device_record_from_target)
        .ok_or_else(|| AppError::BadRequest("Select a receiver first.".to_string()))?;

    if !is_receiver_online(state, &target) {
        return Err(AppError::BadRequest(
            "The selected receiver is not currently available.".to_string(),
        ));
    }

    Ok(target)
}

async fn deliver_remote_playback_command(
    state: &AppState,
    auth: &AuthContext,
    target: &ReceiverDeviceRecord,
    source: PlaybackSourceResponse,
) -> Result<RemotePlaybackCommandResponse, AppError> {
    let queued = sqlx::query_as::<_, RemotePlaybackCommandResponse>(
        r#"
        INSERT INTO receiver_commands (
          user_id, controller_session_id, receiver_device_id, command_type, source_title, status, payload
        )
        VALUES ($1, $2, $3, 'play', $4, 'queued', $5::jsonb)
        RETURNING id, receiver_device_id AS target_device_id, $6 AS target_device_name, command_type, status, source_title, created_at
        "#,
    )
    .bind(auth.user_id)
    .bind(auth.session_id)
    .bind(target.id)
    .bind(&source.title)
    .bind(serde_json::to_value(&source).map_err(|error| AppError::Internal(anyhow!(error)))?)
    .bind(&target.device_name)
    .fetch_one(&state.pool)
    .await?;

    let event = ReceiverEventPayload {
        event_type: "playback_command".to_string(),
        command: queued.clone(),
        source: Some(source),
        position_seconds: None,
        receiver_credential: None,
    };
    if receiver_sender(state, target.id).send(event).is_err() {
        return Err(AppError::BadRequest(
            "The selected receiver is not currently connected.".to_string(),
        ));
    }

    sqlx::query_as::<_, RemotePlaybackCommandResponse>(
        r#"
        UPDATE receiver_commands
        SET status = 'delivered', delivered_at = NOW()
        WHERE id = $1
        RETURNING id, receiver_device_id AS target_device_id, $2 AS target_device_name, command_type, status, source_title, created_at
        "#,
    )
    .bind(queued.id)
    .bind(&target.device_name)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::from)
}

async fn refresh_pairing_code(
    pool: &PgPool,
    receiver_device_id: Uuid,
) -> Result<ReceiverPairingCodeRecord, AppError> {
    sqlx::query("DELETE FROM receiver_pairing_codes WHERE receiver_device_id = $1 OR expires_at <= NOW() OR claimed_at IS NOT NULL")
        .bind(receiver_device_id)
        .execute(pool)
        .await?;

    for _ in 0..10 {
        let code = generate_pairing_code();
        let expires_at = Utc::now() + ChronoDuration::minutes(RECEIVER_PAIRING_CODE_MINUTES);
        let inserted = sqlx::query_as::<_, ReceiverPairingCodeRecord>(
            r#"
            INSERT INTO receiver_pairing_codes (receiver_device_id, code, expires_at, created_at)
            VALUES ($1, $2, $3, NOW())
            ON CONFLICT DO NOTHING
            RETURNING id, receiver_device_id, code, expires_at
            "#,
        )
        .bind(receiver_device_id)
        .bind(&code)
        .bind(expires_at)
        .fetch_optional(pool)
        .await?;
        if let Some(record) = inserted {
            return Ok(record);
        }
    }

    Err(AppError::Internal(anyhow!("failed to issue pairing code")))
}

async fn deliver_receiver_transport_command(
    state: &AppState,
    auth: &AuthContext,
    target: &ReceiverDeviceRecord,
    command_type: &str,
    position_seconds: Option<f64>,
) -> Result<RemotePlaybackCommandResponse, AppError> {
    let source_title = target
        .current_playback_title
        .clone()
        .unwrap_or_else(|| target.device_name.clone());
    let payload = serde_json::json!({
        "positionSeconds": position_seconds,
    });
    let queued = sqlx::query_as::<_, RemotePlaybackCommandResponse>(
        r#"
        INSERT INTO receiver_commands (
          user_id, controller_session_id, receiver_device_id, command_type, source_title, status, payload
        )
        VALUES ($1, $2, $3, $4, $5, 'queued', $6::jsonb)
        RETURNING id, receiver_device_id AS target_device_id, $7 AS target_device_name, command_type, status, source_title, created_at
        "#,
    )
    .bind(auth.user_id)
    .bind(auth.session_id)
    .bind(target.id)
    .bind(command_type)
    .bind(&source_title)
    .bind(payload)
    .bind(&target.device_name)
    .fetch_one(&state.pool)
    .await?;

    let event = ReceiverEventPayload {
        event_type: "transport_command".to_string(),
        command: queued.clone(),
        source: None,
        position_seconds,
        receiver_credential: None,
    };
    if receiver_sender(state, target.id).send(event).is_err() {
        return Err(AppError::BadRequest(
            "The selected receiver is not currently connected.".to_string(),
        ));
    }

    sqlx::query_as::<_, RemotePlaybackCommandResponse>(
        r#"
        UPDATE receiver_commands
        SET status = 'delivered', delivered_at = NOW()
        WHERE id = $1
        RETURNING id, receiver_device_id AS target_device_id, $2 AS target_device_name, command_type, status, source_title, created_at
        "#,
    )
    .bind(queued.id)
    .bind(&target.device_name)
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::from)
}
