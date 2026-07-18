use super::playback::relay_tokens::validate_relay_token;
use super::playback::resolve::PlaybackSourceResponse;
use super::*;
use std::path::{Path as FilePath, PathBuf};
use std::process::Stdio;
use tokio::process::{Child, Command};

const TRANSCODE_STARTUP_TIMEOUT: Duration = Duration::from_secs(25);
const TRANSCODE_STARTUP_POLL_INTERVAL: Duration = Duration::from_millis(100);
const TRANSCODE_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const TRANSCODE_REAPER_INTERVAL: Duration = Duration::from_secs(15);

#[derive(Default)]
pub(super) struct CastTranscodeManager {
    active: Option<ActiveCastTranscode>,
}

struct ActiveCastTranscode {
    access_token: String,
    receiver_device_id: Uuid,
    source_url: String,
    directory: PathBuf,
    child: Child,
    last_accessed_at: Instant,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StartCastTranscodePayload {
    source_url: String,
    title: String,
    live: bool,
    catchup: bool,
}

pub(super) fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/receiver/transcode",
            post(start_cast_transcode).delete(stop_cast_transcode),
        )
        .route(
            "/transcode/{access_token}/{file_name}",
            get(serve_cast_transcode_file),
        )
}

pub(super) fn spawn_reaper(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(TRANSCODE_REAPER_INTERVAL);
        loop {
            interval.tick().await;
            let expired = {
                let mut manager = state.cast_transcodes.lock().await;
                let should_expire = manager
                    .active
                    .as_ref()
                    .map(|active| active.last_accessed_at.elapsed() >= TRANSCODE_IDLE_TIMEOUT)
                    .unwrap_or(false);
                should_expire.then(|| manager.active.take()).flatten()
            };
            if let Some(active) = expired {
                info!(
                    receiver_device_id = %active.receiver_device_id,
                    "stopping idle Cast transcoder"
                );
                terminate_transcode(active).await;
            }
        }
    });
}

pub(super) async fn stop_all(state: &AppState) {
    let active = state.cast_transcodes.lock().await.active.take();
    if let Some(active) = active {
        terminate_transcode(active).await;
    }
}

async fn start_cast_transcode(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<StartCastTranscodePayload>,
) -> ApiResult<PlaybackSourceResponse> {
    if !state.config.cast_transcoding_enabled {
        return Err(AppError::ServiceUnavailable(
            "Cast transcoding is not enabled on this server.".to_string(),
        ));
    }

    let receiver = require_receiver_auth(&state, &headers).await?;
    let device = load_receiver_device(&state.pool, receiver.receiver_device_id)
        .await?
        .ok_or(AppError::Unauthorized)?;
    if device.app_kind != "receiver-google-cast" || device.revoked_at.is_some() {
        return Err(AppError::Forbidden(
            "Cast transcoding is only available to paired Google Cast receivers.".to_string(),
        ));
    }
    let owner_user_id = device.owner_user_id.ok_or(AppError::Unauthorized)?;

    if !payload.live {
        return Err(AppError::BadRequest(
            "Only live Cast streams can be transcoded.".to_string(),
        ));
    }

    let source_url = Url::parse(&payload.source_url).map_err(|_| {
        AppError::BadRequest("The live playback source URL is invalid.".to_string())
    })?;
    if source_url.path() != "/api/relay/hls" {
        return Err(AppError::BadRequest(
            "Only Euripus live HLS relay sources can be transcoded.".to_string(),
        ));
    }
    let relay_token = source_url
        .query_pairs()
        .find_map(|(name, value)| (name == "token").then(|| value.into_owned()))
        .ok_or_else(|| AppError::BadRequest("The relay token is missing.".to_string()))?;
    let relay = validate_relay_token(&state, &relay_token, RelayAssetKind::Hls).await?;
    if relay.user_id != owner_user_id {
        return Err(AppError::Forbidden(
            "The receiver does not own this playback source.".to_string(),
        ));
    }

    let public_base_url = request_base_url(&state.config, &headers)?;
    let source_key = relay.upstream_url.to_string();
    let mut manager = state.cast_transcodes.lock().await;

    let existing_exited = manager
        .active
        .as_mut()
        .map(|active| active.child.try_wait().ok().flatten().is_some())
        .unwrap_or(false);
    if existing_exited {
        if let Some(active) = manager.active.take() {
            remove_transcode_directory(&active.directory).await;
        }
    }

    if let Some(active) = manager.active.as_mut() {
        if active.receiver_device_id != receiver.receiver_device_id {
            return Err(AppError::ServiceUnavailable(
                "Another Cast transcode is already active.".to_string(),
            ));
        }
        if active.source_url == source_key {
            active.last_accessed_at = Instant::now();
            let url = transcode_playlist_url(&public_base_url, &active.access_token)?;
            return Ok(Json(transcoded_source_response(
                &payload,
                url,
                relay.expires_at,
            )));
        }
    }

    if let Some(active) = manager.active.take() {
        terminate_transcode(active).await;
    }

    let access_token = generate_refresh_token();
    let directory = PathBuf::from(&state.config.cast_transcode_directory).join(&access_token);
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|error| AppError::Internal(anyhow!(error)))?;
    let playlist_path = directory.join("index.m3u8");
    let segment_pattern = directory.join("segment-%09d.ts");
    let mut command = build_ffmpeg_command(
        &state.config.cast_transcode_encoder,
        relay.upstream_url.as_str(),
        &playlist_path,
        &segment_pattern,
    );
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            remove_transcode_directory(&directory).await;
            return Err(AppError::ServiceUnavailable(format!(
                "Could not start the Cast transcoder: {error}"
            )));
        }
    };

    let startup_deadline = tokio::time::Instant::now() + TRANSCODE_STARTUP_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| AppError::Internal(anyhow!(error)))?
        {
            remove_transcode_directory(&directory).await;
            return Err(AppError::BadGateway(format!(
                "The Cast transcoder exited during startup with status {status}."
            )));
        }
        if playlist_is_ready(&playlist_path).await {
            break;
        }
        if tokio::time::Instant::now() >= startup_deadline {
            let _ = child.kill().await;
            remove_transcode_directory(&directory).await;
            return Err(AppError::BadGateway(
                "The Cast transcoder did not produce a stream in time.".to_string(),
            ));
        }
        tokio::time::sleep(TRANSCODE_STARTUP_POLL_INTERVAL).await;
    }

    let url = transcode_playlist_url(&public_base_url, &access_token)?;
    info!(
        receiver_device_id = %receiver.receiver_device_id,
        encoder = %state.config.cast_transcode_encoder,
        "started on-demand Cast live transcoder"
    );
    manager.active = Some(ActiveCastTranscode {
        access_token,
        receiver_device_id: receiver.receiver_device_id,
        source_url: source_key,
        directory,
        child,
        last_accessed_at: Instant::now(),
    });

    Ok(Json(transcoded_source_response(
        &payload,
        url,
        relay.expires_at,
    )))
}

async fn stop_cast_transcode(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<StatusCode, AppError> {
    let receiver = require_receiver_auth(&state, &headers).await?;
    let active = {
        let mut manager = state.cast_transcodes.lock().await;
        if manager
            .active
            .as_ref()
            .is_some_and(|active| active.receiver_device_id == receiver.receiver_device_id)
        {
            manager.active.take()
        } else {
            None
        }
    };
    if let Some(active) = active {
        terminate_transcode(active).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn serve_cast_transcode_file(
    State(state): State<AppState>,
    Path((access_token, file_name)): Path<(String, String)>,
) -> Result<Response, AppError> {
    if !valid_transcode_file_name(&file_name) {
        return Err(AppError::NotFound("Transcode file not found.".to_string()));
    }

    let path = {
        let mut manager = state.cast_transcodes.lock().await;
        let active = manager
            .active
            .as_mut()
            .filter(|active| active.access_token == access_token)
            .ok_or_else(|| AppError::NotFound("Transcode session not found.".to_string()))?;
        if active
            .child
            .try_wait()
            .map_err(|error| AppError::Internal(anyhow!(error)))?
            .is_some()
        {
            return Err(AppError::ServiceUnavailable(
                "The Cast transcoder is no longer running.".to_string(),
            ));
        }
        active.last_accessed_at = Instant::now();
        active.directory.join(&file_name)
    };

    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::NotFound => {
                AppError::NotFound("Transcode file not found.".to_string())
            }
            _ => AppError::Internal(anyhow!(error)),
        })?;
    let content_type = if file_name.ends_with(".m3u8") {
        "application/vnd.apple.mpegurl"
    } else {
        "video/mp2t"
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, content_type)
        .header(header::CACHE_CONTROL, "no-store")
        .body(Body::from(bytes))
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

fn build_ffmpeg_command(
    encoder: &str,
    upstream_url: &str,
    playlist_path: &FilePath,
    segment_pattern: &FilePath,
) -> Command {
    let mut command = Command::new("ffmpeg");
    command
        .kill_on_drop(true)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .args([
            "-hide_banner",
            "-loglevel",
            "warning",
            "-nostdin",
            "-reconnect",
            "1",
            "-reconnect_streamed",
            "1",
            "-reconnect_delay_max",
            "5",
            "-user_agent",
            "Euripus Cast Transcoder/1.0",
            "-i",
            upstream_url,
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-vf",
            "scale=w='min(1920,iw)':h='min(1080,ih)':force_original_aspect_ratio=decrease:force_divisible_by=2,fps=30,format=yuv420p",
            "-c:v",
            encoder,
            "-preset",
            "p4",
            "-tune",
            "ll",
            "-profile:v",
            "high",
            "-level:v",
            "4.1",
            "-rc",
            "vbr",
            "-cq",
            "23",
            "-b:v",
            "6000k",
            "-maxrate",
            "8000k",
            "-bufsize",
            "12000k",
            "-g",
            "60",
            "-keyint_min",
            "60",
            "-c:a",
            "aac",
            "-b:a",
            "160k",
            "-ac",
            "2",
            "-ar",
            "48000",
            "-f",
            "hls",
            "-hls_time",
            "2",
            "-hls_list_size",
            "8",
            "-hls_delete_threshold",
            "4",
            "-hls_flags",
            "delete_segments+omit_endlist+independent_segments+temp_file",
            "-hls_segment_filename",
        ])
        .arg(segment_pattern)
        .arg(playlist_path);
    command
}

fn transcoded_source_response(
    payload: &StartCastTranscodePayload,
    url: String,
    expires_at: DateTime<Utc>,
) -> PlaybackSourceResponse {
    PlaybackSourceResponse {
        kind: "hls".to_string(),
        url,
        headers: HashMap::new(),
        live: true,
        catchup: payload.catchup,
        expires_at: Some(expires_at),
        unsupported_reason: None,
        title: payload.title.chars().take(512).collect(),
    }
}

fn transcode_playlist_url(base_url: &Url, access_token: &str) -> Result<String, AppError> {
    base_url
        .join(&format!("/api/transcode/{access_token}/index.m3u8"))
        .map(|url| url.to_string())
        .map_err(|error| AppError::Internal(anyhow!(error)))
}

async fn playlist_is_ready(path: &FilePath) -> bool {
    tokio::fs::read_to_string(path)
        .await
        .map(|playlist| playlist.contains("segment-") && playlist.contains("#EXTINF"))
        .unwrap_or(false)
}

fn valid_transcode_file_name(file_name: &str) -> bool {
    file_name == "index.m3u8"
        || (file_name.starts_with("segment-")
            && file_name.ends_with(".ts")
            && file_name
                .trim_start_matches("segment-")
                .trim_end_matches(".ts")
                .chars()
                .all(|character| character.is_ascii_digit()))
}

async fn terminate_transcode(mut active: ActiveCastTranscode) {
    let _ = active.child.kill().await;
    remove_transcode_directory(&active.directory).await;
}

async fn remove_transcode_directory(directory: &FilePath) {
    if let Err(error) = tokio::fs::remove_dir_all(directory).await
        && error.kind() != std::io::ErrorKind::NotFound
    {
        warn!(path = %directory.display(), error = ?error, "failed to remove Cast transcode directory");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_only_generated_transcode_file_names() {
        assert!(valid_transcode_file_name("index.m3u8"));
        assert!(valid_transcode_file_name("segment-000000001.ts"));
        assert!(!valid_transcode_file_name("../index.m3u8"));
        assert!(!valid_transcode_file_name("segment-anything.ts"));
        assert!(!valid_transcode_file_name("other.ts"));
    }

    #[test]
    fn configures_nvenc_for_chromecast_compatible_hls() {
        let command = build_ffmpeg_command(
            "h264_nvenc",
            "https://provider.example.com/live.m3u8",
            FilePath::new("/tmp/output/index.m3u8"),
            FilePath::new("/tmp/output/segment-%09d.ts"),
        );
        let args = command
            .as_std()
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let joined = args.join(" ");

        assert!(joined.contains("-c:v h264_nvenc"));
        assert!(joined.contains("-profile:v high"));
        assert!(joined.contains("-level:v 4.1"));
        assert!(joined.contains("fps=30"));
        assert!(joined.contains("-hls_time 2"));
    }

    #[test]
    fn builds_private_playlist_url() {
        let base = Url::parse("https://tv.example.com").expect("base URL");
        let url = transcode_playlist_url(&base, "secret-token").expect("playlist URL");
        assert_eq!(
            url,
            "https://tv.example.com/api/transcode/secret-token/index.m3u8"
        );
    }
}
