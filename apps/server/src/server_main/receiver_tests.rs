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
