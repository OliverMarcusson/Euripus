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
    let format = resolve_effective_playback_format("m3u8", Some("ts")).expect("playback format");

    assert_eq!(format, PlaybackStreamFormat::Ts);
}

#[test]
fn resolve_effective_playback_format_uses_saved_output_format_when_channel_extension_missing() {
    let format = resolve_effective_playback_format("m3u8", None).expect("playback format");

    assert_eq!(format, PlaybackStreamFormat::Hls);
}

#[test]
fn resolve_effective_playback_format_for_browser_forces_hls() {
    let format =
        resolve_effective_playback_format_for_target(PlaybackTarget::Browser, "ts", Some("ts"))
            .expect("browser playback format");

    assert_eq!(format, PlaybackStreamFormat::Hls);
}

#[test]
fn resolve_effective_playback_format_for_cast_forces_hls() {
    let format =
        resolve_effective_playback_format_for_target(PlaybackTarget::Cast, "ts", Some("ts"))
            .expect("cast playback format");

    assert_eq!(format, PlaybackStreamFormat::Hls);
}

#[test]
fn resolve_effective_playback_format_falls_back_to_legacy_stream_extension() {
    let format = resolve_effective_playback_format("legacy", Some("ts")).expect("playback format");

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
        ProgramPlaybackBehavior::Unsupported("This program is not mapped to a playable channel.",)
    );
}
