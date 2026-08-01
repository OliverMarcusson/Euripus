use self::resolve::{
    PlaybackSourceResponse, PlaybackStreamFormat, PlaybackTarget, ProgramPlaybackBehavior,
    ProgramPlaybackRow, determine_program_playback_behavior, output_format_as_str,
    playback_source_for_mode, resolve_effective_playback_format,
    resolve_effective_playback_format_for_target, unsupported_playback,
};
use super::*;

pub(super) mod relay_tokens;
pub(super) mod resolve;

const BROWSER_HLS_UNSUPPORTED_REASON: &str = "This provider stream could not be verified for browser HLS playback. Try a receiver/native target instead.";

#[derive(Debug, Default, Deserialize)]
struct PlaybackQuery {
    target: Option<String>,
}

impl PlaybackQuery {
    fn target(&self) -> Result<PlaybackTarget, AppError> {
        match self.target.as_deref() {
            None | Some("") => Ok(PlaybackTarget::Browser),
            Some("cast") => Ok(PlaybackTarget::Cast),
            Some(_) => Err(AppError::BadRequest(
                "Playback target must be 'cast' when specified.".to_string(),
            )),
        }
    }
}

#[derive(Debug, FromRow)]
struct ChannelPlaybackRecord {
    id: Uuid,
    profile_id: Uuid,
    name: String,
    remote_stream_id: i32,
    stream_extension: Option<String>,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
    playback_mode: String,
}

#[derive(Debug, FromRow)]
struct OnDemandPlaybackRecord {
    profile_id: Uuid,
    title_id: Uuid,
    episode_id: Option<Uuid>,
    name: String,
    media_type: String,
    remote_id: String,
    container_extension: Option<String>,
    base_url: String,
    provider_username: String,
    password_encrypted: String,
    output_format: String,
    playback_mode: String,
}

pub(super) fn shared_router() -> Router<AppState> {
    Router::new()
        .route("/playback/channel/{id}", post(play_channel))
        .route("/playback/program/{id}", post(play_program))
        .route("/playback/on-demand/{id}", post(play_on_demand))
        .route("/playback/episode/{id}", post(play_episode))
}

async fn play_channel(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlaybackQuery>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    Ok(Json(
        resolve_channel_playback_source_for_target(
            &state,
            &headers,
            None,
            auth.user_id,
            id,
            query.target()?,
        )
        .await?,
    ))
}

async fn play_program(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlaybackQuery>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    Ok(Json(
        resolve_program_playback_source_for_target(
            &state,
            &headers,
            None,
            auth.user_id,
            id,
            query.target()?,
        )
        .await?,
    ))
}

async fn play_on_demand(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlaybackQuery>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    Ok(Json(
        resolve_on_demand_playback_source_for_target(
            &state,
            &headers,
            None,
            auth.user_id,
            id,
            false,
            query.target()?,
        )
        .await?,
    ))
}

async fn play_episode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlaybackQuery>,
    Path(id): Path<Uuid>,
) -> ApiResult<PlaybackSourceResponse> {
    let auth = require_auth(&state, &headers).await?;
    Ok(Json(
        resolve_on_demand_playback_source_for_target(
            &state,
            &headers,
            None,
            auth.user_id,
            id,
            true,
            query.target()?,
        )
        .await?,
    ))
}

pub(in crate::server_main) async fn resolve_on_demand_playback_source_for_receiver(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    episode: bool,
    receiver_app_kind: &str,
    receiver_public_origin: Option<&str>,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_on_demand_playback_source_for_target(
        state,
        headers,
        receiver_public_origin,
        user_id,
        id,
        episode,
        playback_target_for_receiver_app(receiver_app_kind),
    )
    .await
}

async fn resolve_on_demand_playback_source_for_target(
    state: &AppState,
    headers: &HeaderMap,
    target_public_origin: Option<&str>,
    user_id: Uuid,
    id: Uuid,
    episode: bool,
    target: PlaybackTarget,
) -> Result<PlaybackSourceResponse, AppError> {
    let query = if episode {
        r#"SELECT e.profile_id, e.series_id AS title_id, e.id AS episode_id,
          e.name, 'series'::text AS media_type, e.remote_id, e.container_extension,
          p.base_url, p.username AS provider_username, p.password_encrypted, p.output_format, p.playback_mode
          FROM on_demand_episodes e JOIN provider_profiles p ON p.id=e.profile_id
          WHERE e.user_id=$1 AND e.id=$2"#
    } else {
        r#"SELECT t.profile_id, t.id AS title_id, NULL::uuid AS episode_id,
          t.name, t.media_type, t.remote_id, t.container_extension,
          p.base_url, p.username AS provider_username, p.password_encrypted, p.output_format, p.playback_mode
          FROM on_demand_titles t JOIN provider_profiles p ON p.id=t.profile_id
          WHERE t.user_id=$1 AND t.id=$2 AND t.media_type='movie'"#
    };
    let row = sqlx::query_as::<_, OnDemandPlaybackRecord>(query)
        .bind(user_id)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("On-demand item not found".to_string()))?;
    sqlx::query(
        r#"INSERT INTO on_demand_playback_history (user_id, title_id, episode_id, last_played_at)
           VALUES ($1, $2, $3, NOW())
           ON CONFLICT (user_id, title_id) DO UPDATE SET
             episode_id=EXCLUDED.episode_id, last_played_at=NOW()"#,
    )
    .bind(user_id)
    .bind(row.title_id)
    .bind(row.episode_id)
    .execute(&state.pool)
    .await?;
    let extension = row
        .container_extension
        .as_deref()
        .unwrap_or("mp4")
        .trim_start_matches('.')
        .to_ascii_lowercase();
    let credentials = XtreamCredentials {
        base_url: row.base_url,
        username: row.provider_username,
        password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
        output_format: row.output_format,
    };
    let url = xtreme::build_on_demand_stream_url(
        &credentials,
        &row.media_type,
        &row.remote_id,
        Some(&extension),
    )?;
    finalize_playback_source(
        state,
        headers,
        target_public_origin,
        user_id,
        row.profile_id,
        target,
        &row.playback_mode,
        &row.name,
        url,
        false,
        false,
        PlaybackStreamFormat::Progressive,
        None,
        false,
    )
    .await
}

pub(in crate::server_main) async fn resolve_channel_playback_source_for_receiver(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    receiver_app_kind: &str,
    receiver_public_origin: Option<&str>,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_channel_playback_source_for_target(
        state,
        headers,
        receiver_public_origin,
        user_id,
        id,
        playback_target_for_receiver_app(receiver_app_kind),
    )
    .await
}

async fn resolve_channel_playback_source_for_target(
    state: &AppState,
    headers: &HeaderMap,
    target_public_origin: Option<&str>,
    user_id: Uuid,
    id: Uuid,
    target: PlaybackTarget,
) -> Result<PlaybackSourceResponse, AppError> {
    let record = sqlx::query_as::<_, ChannelPlaybackRecord>(
        r#"
        SELECT
          c.id,
          c.profile_id,
          c.name,
          c.remote_stream_id,
          c.stream_extension,
          p.base_url,
          p.username AS provider_username,
          p.password_encrypted,
          p.output_format,
          p.playback_mode
        FROM channels c
        JOIN provider_profiles p ON p.id = c.profile_id
        WHERE c.user_id = $1 AND c.id = $2
          AND c.profile_id = (SELECT live_provider_id FROM users WHERE id = $1)
        "#,
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Channel not found".to_string()))?;

    ensure_channel_is_visible(state, user_id, record.profile_id, record.id).await?;
    let credentials = playback_credentials(state, &record)?;
    let browser_hls_preflight_required = target_requires_browser_hls_preflight(
        target,
        &record.output_format,
        record.stream_extension.as_deref(),
    );
    let format = resolve_effective_playback_format_for_target(
        target,
        &record.output_format,
        record.stream_extension.as_deref(),
    )?;
    let url = xtreme::build_live_stream_url(
        &credentials,
        record.remote_stream_id,
        Some(output_format_as_str(format)),
    )?;
    touch_recent(&state.pool, user_id, record.id).await?;

    finalize_playback_source(
        state,
        headers,
        target_public_origin,
        user_id,
        record.profile_id,
        target,
        &record.playback_mode,
        &record.name,
        url,
        true,
        false,
        format,
        None,
        browser_hls_preflight_required,
    )
    .await
}

pub(in crate::server_main) async fn resolve_program_playback_source_for_receiver(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    id: Uuid,
    receiver_app_kind: &str,
    receiver_public_origin: Option<&str>,
) -> Result<PlaybackSourceResponse, AppError> {
    resolve_program_playback_source_for_target(
        state,
        headers,
        receiver_public_origin,
        user_id,
        id,
        playback_target_for_receiver_app(receiver_app_kind),
    )
    .await
}

async fn resolve_program_playback_source_for_target(
    state: &AppState,
    headers: &HeaderMap,
    target_public_origin: Option<&str>,
    user_id: Uuid,
    id: Uuid,
    target: PlaybackTarget,
) -> Result<PlaybackSourceResponse, AppError> {
    let row = sqlx::query_as::<_, ProgramPlaybackRow>(
        r#"
        SELECT
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
          AND p.profile_id = (SELECT live_provider_id FROM users WHERE id = $1)
        "#,
    )
    .bind(user_id)
    .bind(id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Program not found".to_string()))?;

    let behavior = determine_program_playback_behavior(&row, Utc::now());

    let Some(channel_id) = row.channel_id else {
        return Ok(unsupported_playback(
            &row.title,
            "This program is not mapped to a playable channel.",
        ));
    };
    ensure_channel_is_visible(state, user_id, row.profile_id, channel_id).await?;
    touch_recent(&state.pool, user_id, channel_id).await?;

    match behavior {
        ProgramPlaybackBehavior::Live => {
            let credentials = XtreamCredentials {
                base_url: row.base_url,
                username: row.provider_username,
                password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
                output_format: row.output_format,
            };
            let browser_hls_preflight_required = target_requires_browser_hls_preflight(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            );
            let format = resolve_effective_playback_format_for_target(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            )?;
            let url = xtreme::build_live_stream_url(
                &credentials,
                row.remote_stream_id,
                Some(output_format_as_str(format)),
            )?;

            finalize_playback_source(
                state,
                headers,
                target_public_origin,
                user_id,
                row.profile_id,
                target,
                &row.playback_mode,
                &row.channel_name,
                url,
                true,
                false,
                format,
                None,
                browser_hls_preflight_required,
            )
            .await
        }
        ProgramPlaybackBehavior::Catchup => {
            let credentials = XtreamCredentials {
                base_url: row.base_url,
                username: row.provider_username,
                password: decrypt_secret(&state.config.encryption_key, &row.password_encrypted)?,
                output_format: row.output_format,
            };
            let browser_hls_preflight_required = target_requires_browser_hls_preflight(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            );
            let format = resolve_effective_playback_format_for_target(
                target,
                &credentials.output_format,
                row.stream_extension.as_deref(),
            )?;
            let url = xtreme::build_catchup_url(
                &credentials,
                row.remote_stream_id,
                Some(output_format_as_str(format)),
                row.start_at,
                row.end_at,
            )?;

            finalize_playback_source(
                state,
                headers,
                target_public_origin,
                user_id,
                row.profile_id,
                target,
                &row.playback_mode,
                &row.title,
                url,
                false,
                true,
                format,
                None,
                browser_hls_preflight_required,
            )
            .await
        }
        ProgramPlaybackBehavior::Unsupported(reason) => {
            Ok(unsupported_playback(&row.title, reason))
        }
    }
}

async fn ensure_channel_is_visible(
    state: &AppState,
    user_id: Uuid,
    profile_id: Uuid,
    channel_id: Uuid,
) -> Result<(), AppError> {
    let visibility = load_channel_visibility_map(state, user_id, Some(profile_id)).await?;
    if visibility
        .get(&channel_id)
        .is_some_and(|value| value.is_hidden)
    {
        return Err(AppError::NotFound("Channel not found".to_string()));
    }
    Ok(())
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

fn playback_target_for_receiver_app(app_kind: &str) -> PlaybackTarget {
    match app_kind {
        "receiver-android-tv" => PlaybackTarget::ReceiverAndroidTv,
        "receiver-google-cast" => PlaybackTarget::Cast,
        _ => PlaybackTarget::ReceiverWeb,
    }
}

fn target_requires_browser_hls_preflight(
    target: PlaybackTarget,
    output_format: &str,
    legacy_stream_extension: Option<&str>,
) -> bool {
    matches!(target, PlaybackTarget::Browser)
        && !matches!(
            resolve_effective_playback_format(output_format, legacy_stream_extension),
            Ok(PlaybackStreamFormat::Hls)
        )
}

pub(super) async fn finalize_playback_source(
    state: &AppState,
    headers: &HeaderMap,
    target_public_origin: Option<&str>,
    user_id: Uuid,
    profile_id: Uuid,
    target: PlaybackTarget,
    raw_playback_mode: &str,
    title: &str,
    upstream_url: String,
    live: bool,
    catchup: bool,
    format: PlaybackStreamFormat,
    expires_at: Option<DateTime<Utc>>,
    browser_hls_preflight_required: bool,
) -> Result<PlaybackSourceResponse, AppError> {
    if browser_hls_preflight_required {
        match xtreme::probe_hls_playlist_url(&state.provider_http_client, &upstream_url).await {
            Ok(true) => {}
            Ok(false) => {
                warn!(title = %title, upstream_url = %upstream_url, "browser HLS preflight failed");
                return Ok(unsupported_playback(title, BROWSER_HLS_UNSUPPORTED_REASON));
            }
            Err(error) => {
                warn!(
                    title = %title,
                    upstream_url = %upstream_url,
                    error = ?error,
                    "browser HLS preflight errored"
                );
                return Ok(unsupported_playback(title, BROWSER_HLS_UNSUPPORTED_REASON));
            }
        }
    }

    playback_source_for_mode(
        state,
        headers,
        target_public_origin,
        user_id,
        profile_id,
        target,
        raw_playback_mode,
        title,
        upstream_url,
        live,
        catchup,
        format,
        expires_at,
    )
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
