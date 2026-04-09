use super::relay_tokens::{issue_relay_token, relay_url_for_token};
use super::*;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub(in crate::server_main) struct PlaybackSourceResponse {
    pub(in crate::server_main) kind: String,
    pub(in crate::server_main) url: String,
    pub(in crate::server_main) headers: HashMap<String, String>,
    pub(in crate::server_main) live: bool,
    pub(in crate::server_main) catchup: bool,
    pub(in crate::server_main) expires_at: Option<DateTime<Utc>>,
    pub(in crate::server_main) unsupported_reason: Option<String>,
    pub(in crate::server_main) title: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::server_main) enum PlaybackMode {
    Direct,
    Relay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::server_main) enum PlaybackTarget {
    Browser,
    ReceiverWeb,
    ReceiverAndroidTv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::server_main) enum PlaybackStreamFormat {
    Hls,
    Ts,
}

#[derive(Debug, FromRow)]
pub(in crate::server_main) struct ProgramPlaybackRow {
    pub(in crate::server_main) title: String,
    pub(in crate::server_main) start_at: DateTime<Utc>,
    pub(in crate::server_main) end_at: DateTime<Utc>,
    pub(in crate::server_main) can_catchup: bool,
    pub(in crate::server_main) profile_id: Uuid,
    pub(in crate::server_main) channel_id: Option<Uuid>,
    pub(in crate::server_main) remote_stream_id: i32,
    pub(in crate::server_main) stream_extension: Option<String>,
    pub(in crate::server_main) channel_name: String,
    pub(in crate::server_main) has_catchup: bool,
    pub(in crate::server_main) base_url: String,
    pub(in crate::server_main) provider_username: String,
    pub(in crate::server_main) password_encrypted: String,
    pub(in crate::server_main) output_format: String,
    pub(in crate::server_main) playback_mode: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(in crate::server_main) enum ProgramPlaybackBehavior {
    Live,
    Catchup,
    Unsupported(&'static str),
}

pub(in crate::server_main) fn normalize_playback_mode(raw: &str) -> Result<PlaybackMode, AppError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "direct" => Ok(PlaybackMode::Direct),
        "relay" => Ok(PlaybackMode::Relay),
        _ => Err(AppError::BadRequest(
            "Playback mode must be either 'direct' or 'relay'.".to_string(),
        )),
    }
}

pub(in crate::server_main) fn playback_mode_as_str(mode: PlaybackMode) -> &'static str {
    match mode {
        PlaybackMode::Direct => "direct",
        PlaybackMode::Relay => "relay",
    }
}

pub(in crate::server_main) fn normalize_output_format(
    raw: &str,
) -> Result<PlaybackStreamFormat, AppError> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "m3u8" => Ok(PlaybackStreamFormat::Hls),
        "ts" => Ok(PlaybackStreamFormat::Ts),
        _ => Err(AppError::BadRequest(
            "Output format must be either 'm3u8' or 'ts'.".to_string(),
        )),
    }
}

pub(in crate::server_main) fn output_format_as_str(format: PlaybackStreamFormat) -> &'static str {
    match format {
        PlaybackStreamFormat::Hls => "m3u8",
        PlaybackStreamFormat::Ts => "ts",
    }
}

pub(in crate::server_main) fn unsupported_playback(
    title: &str,
    reason: &str,
) -> PlaybackSourceResponse {
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

pub(in crate::server_main) fn playback_source_for_mode(
    state: &AppState,
    headers: &HeaderMap,
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
) -> Result<PlaybackSourceResponse, AppError> {
    let direct = playback_source_from_url(
        title,
        upstream_url.clone(),
        live,
        catchup,
        format,
        expires_at,
    );
    let playback_mode = normalize_playback_mode(raw_playback_mode)?;

    let request_base_url = request_base_url(&state.config, headers)?;
    let relay_required_for_android_tv = matches!(target, PlaybackTarget::ReceiverAndroidTv);
    let relay_required_for_https = matches!(target, PlaybackTarget::Browser)
        && should_force_relay_for_secure_request(&request_base_url, &upstream_url);
    let bypass_relay_in_local_dev = matches!(target, PlaybackTarget::Browser)
        && playback_mode == PlaybackMode::Relay
        && state.config.public_origin.is_none()
        && !state.config.vpn_enabled
        && !relay_required_for_https;
    if (playback_mode == PlaybackMode::Direct
        && !relay_required_for_https
        && !relay_required_for_android_tv)
        || bypass_relay_in_local_dev
    {
        return Ok(direct);
    }

    let relay_kind = relay_asset_kind_for_format(format);
    let relay_token =
        issue_relay_token(state, user_id, profile_id, &upstream_url, relay_kind, None)?;
    let relay_url = relay_url_for_token(&request_base_url, relay_kind, &relay_token.token)?;

    Ok(PlaybackSourceResponse {
        url: relay_url,
        expires_at: Some(relay_token.expires_at),
        ..direct
    })
}

pub(in crate::server_main) fn should_force_relay_for_secure_request(
    request_base_url: &Url,
    upstream_url: &str,
) -> bool {
    if request_base_url.scheme() != "https" {
        return false;
    }

    Url::parse(upstream_url)
        .map(|url| url.scheme() == "http")
        .unwrap_or(false)
}

pub(in crate::server_main) fn playback_source_from_url(
    title: &str,
    url: String,
    live: bool,
    catchup: bool,
    format: PlaybackStreamFormat,
    expires_at: Option<DateTime<Utc>>,
) -> PlaybackSourceResponse {
    PlaybackSourceResponse {
        kind: playback_kind_for_format(format).to_string(),
        url,
        headers: HashMap::new(),
        live,
        catchup,
        expires_at,
        unsupported_reason: None,
        title: title.to_string(),
    }
}

pub(in crate::server_main) fn resolve_effective_playback_format(
    output_format: &str,
    legacy_stream_extension: Option<&str>,
) -> Result<PlaybackStreamFormat, AppError> {
    if let Some(format) = legacy_stream_extension
        .map(normalize_output_format)
        .transpose()?
    {
        return Ok(format);
    }

    normalize_output_format(output_format).or_else(|_| {
        Err(AppError::BadRequest(
            "The provider returned a stream format Euripus v1 cannot play.".to_string(),
        ))
    })
}

pub(in crate::server_main) fn resolve_effective_playback_format_for_target(
    target: PlaybackTarget,
    output_format: &str,
    legacy_stream_extension: Option<&str>,
) -> Result<PlaybackStreamFormat, AppError> {
    if matches!(target, PlaybackTarget::Browser) {
        return Ok(PlaybackStreamFormat::Hls);
    }

    resolve_effective_playback_format(output_format, legacy_stream_extension)
}

pub(in crate::server_main) fn playback_kind_for_format(
    format: PlaybackStreamFormat,
) -> &'static str {
    match format {
        PlaybackStreamFormat::Hls => "hls",
        PlaybackStreamFormat::Ts => "mpegts",
    }
}

pub(in crate::server_main) fn relay_asset_kind_for_format(
    format: PlaybackStreamFormat,
) -> RelayAssetKind {
    match format {
        PlaybackStreamFormat::Hls => RelayAssetKind::Hls,
        PlaybackStreamFormat::Ts => RelayAssetKind::Raw,
    }
}

pub(in crate::server_main) fn determine_program_playback_behavior(
    row: &ProgramPlaybackRow,
    now: DateTime<Utc>,
) -> ProgramPlaybackBehavior {
    if row.channel_id.is_none() {
        return ProgramPlaybackBehavior::Unsupported(
            "This program is not mapped to a playable channel.",
        );
    }

    if row.start_at <= now && row.end_at > now {
        return ProgramPlaybackBehavior::Live;
    }

    if row.end_at <= now && row.can_catchup && row.has_catchup {
        return ProgramPlaybackBehavior::Catchup;
    }

    ProgramPlaybackBehavior::Unsupported(
        "Catch-up is not available for this program on the provider.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_program_playback_row(
        start_at: DateTime<Utc>,
        end_at: DateTime<Utc>,
    ) -> ProgramPlaybackRow {
        ProgramPlaybackRow {
            title: "Matchday Live".to_string(),
            start_at,
            end_at,
            can_catchup: true,
            profile_id: Uuid::from_u128(9),
            channel_id: Some(Uuid::from_u128(8)),
            remote_stream_id: 42,
            stream_extension: Some("m3u8".to_string()),
            channel_name: "Arena 1".to_string(),
            has_catchup: true,
            base_url: "https://provider.example.com".to_string(),
            provider_username: "demo".to_string(),
            password_encrypted: "encrypted".to_string(),
            output_format: "m3u8".to_string(),
            playback_mode: "direct".to_string(),
        }
    }

    #[test]
    fn produces_hls_kind_for_m3u8_urls() {
        let response = playback_source_from_url(
            "News",
            "https://example.com/live.m3u8".to_string(),
            true,
            false,
            PlaybackStreamFormat::Hls,
            None,
        );
        assert_eq!(response.kind, "hls");
    }

    #[test]
    fn resolve_effective_playback_format_prefers_channel_stream_extension() {
        let format =
            resolve_effective_playback_format("m3u8", Some("ts")).expect("playback format");

        assert_eq!(format, PlaybackStreamFormat::Ts);
    }

    #[test]
    fn resolve_effective_playback_format_uses_saved_output_format_when_channel_extension_missing() {
        let format = resolve_effective_playback_format("m3u8", None).expect("playback format");

        assert_eq!(format, PlaybackStreamFormat::Hls);
    }

    #[test]
    fn resolve_effective_playback_format_for_browser_forces_hls() {
        let format = resolve_effective_playback_format_for_target(
            PlaybackTarget::Browser,
            "ts",
            Some("ts"),
        )
        .expect("browser playback format");

        assert_eq!(format, PlaybackStreamFormat::Hls);
    }

    #[test]
    fn resolve_effective_playback_format_falls_back_to_legacy_stream_extension() {
        let format =
            resolve_effective_playback_format("legacy", Some("ts")).expect("playback format");

        assert_eq!(format, PlaybackStreamFormat::Ts);
    }

    #[test]
    fn program_playback_uses_live_channel_when_program_is_airing() {
        let now = Utc::now();
        let row = sample_program_playback_row(
            now - ChronoDuration::minutes(15),
            now + ChronoDuration::minutes(45),
        );

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(behavior, ProgramPlaybackBehavior::Live);
    }

    #[test]
    fn program_playback_uses_catchup_when_program_has_ended_and_archive_is_available() {
        let now = Utc::now();
        let row = sample_program_playback_row(
            now - ChronoDuration::hours(2),
            now - ChronoDuration::hours(1),
        );

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(behavior, ProgramPlaybackBehavior::Catchup);
    }

    #[test]
    fn program_playback_is_unsupported_for_upcoming_programs() {
        let now = Utc::now();
        let row = sample_program_playback_row(
            now + ChronoDuration::minutes(10),
            now + ChronoDuration::minutes(70),
        );

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(
            behavior,
            ProgramPlaybackBehavior::Unsupported(
                "Catch-up is not available for this program on the provider.",
            )
        );
    }

    #[test]
    fn program_playback_is_unsupported_when_program_is_not_mapped_to_a_channel() {
        let now = Utc::now();
        let mut row = sample_program_playback_row(
            now - ChronoDuration::minutes(15),
            now + ChronoDuration::minutes(45),
        );
        row.channel_id = None;

        let behavior = determine_program_playback_behavior(&row, now);

        assert_eq!(
            behavior,
            ProgramPlaybackBehavior::Unsupported(
                "This program is not mapped to a playable channel.",
            )
        );
    }
}
