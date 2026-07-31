use super::*;

pub(super) async fn pair_receiver(
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

pub(super) async fn list_remote_receivers(
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

pub(super) async fn unpair_receiver(
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

pub(super) async fn get_remote_controller_target(
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

pub(super) async fn select_remote_controller_target(
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

pub(super) async fn clear_remote_controller_target(
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

pub(super) async fn play_channel_remotely(
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

pub(super) async fn play_program_remotely(
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

pub(super) async fn play_on_demand_remotely(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<RemotePlaybackCommandResponse> {
    play_on_demand_item_remotely(state, headers, id, false).await
}

pub(super) async fn play_episode_remotely(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> ApiResult<RemotePlaybackCommandResponse> {
    play_on_demand_item_remotely(state, headers, id, true).await
}

async fn play_on_demand_item_remotely(
    state: AppState,
    headers: HeaderMap,
    id: Uuid,
    episode: bool,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    let source = resolve_on_demand_playback_source_for_receiver(
        &state,
        &headers,
        auth.user_id,
        id,
        episode,
        &target.app_kind,
        target.last_public_origin.as_deref(),
    )
    .await?;
    Ok(Json(
        deliver_remote_playback_command(&state, &auth, &target, source).await?,
    ))
}

pub(super) async fn pause_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(&state, &auth, &target, "pause", None).await?,
    ))
}

pub(super) async fn resume_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(&state, &auth, &target, "play", None).await?,
    ))
}

pub(super) async fn seek_remote_playback(
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

pub(super) async fn stop_remote_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<RemotePlaybackCommandResponse> {
    let auth = require_auth(&state, &headers).await?;
    let target = current_remote_target_for_control(&state, &auth).await?;
    Ok(Json(
        deliver_receiver_transport_command(&state, &auth, &target, "stop", None).await?,
    ))
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
