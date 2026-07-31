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
        true,
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

fn joined_args(command: &Command) -> String {
    command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn paces_on_demand_input_and_keeps_a_longer_segment_window() {
    let on_demand = build_ffmpeg_command(
        "h264_nvenc",
        "https://provider.example.com/movie.mkv",
        FilePath::new("/tmp/output/index.m3u8"),
        FilePath::new("/tmp/output/segment-%09d.ts"),
        false,
    );
    let joined = joined_args(&on_demand);

    // Without -re the GPU races past the window and deletes segments the
    // player has not reached yet.
    assert!(joined.contains("-re -i https://provider.example.com/movie.mkv"));
    assert!(joined.contains("-hls_list_size 150"));
    assert!(joined.contains("-hls_time 4"));
    // Still a rolling window, so disk stays bounded.
    assert!(joined.contains("delete_segments"));
}

#[test]
fn does_not_pace_a_live_upstream() {
    let live = build_ffmpeg_command(
        "h264_nvenc",
        "https://provider.example.com/live.m3u8",
        FilePath::new("/tmp/output/index.m3u8"),
        FilePath::new("/tmp/output/segment-%09d.ts"),
        true,
    );
    let joined = joined_args(&live);

    assert!(!joined.contains("-re "));
    assert!(joined.contains("-hls_list_size 8"));
}

#[test]
fn tunes_each_encoder_with_flags_it_accepts() {
    assert_eq!(
        encoder_tuning_args("h264_nvenc"),
        ["-preset", "p4", "-tune", "ll", "-rc", "vbr", "-cq", "23"]
    );
    assert_eq!(
        encoder_tuning_args("libx264"),
        ["-preset", "veryfast", "-crf", "23"]
    );
    assert!(encoder_tuning_args("h264_vaapi").is_empty());
}

#[test]
fn omits_nvenc_only_rate_control_for_software_encoding() {
    let command = build_ffmpeg_command(
        "libx264",
        "https://provider.example.com/live.m3u8",
        FilePath::new("/tmp/output/index.m3u8"),
        FilePath::new("/tmp/output/segment-%09d.ts"),
        true,
    );
    let joined = command
        .as_std()
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(joined.contains("-c:v libx264"));
    assert!(joined.contains("-preset veryfast"));
    assert!(!joined.contains("-rc vbr"));
    assert!(!joined.contains("-cq 23"));
    // Portable flags stay regardless of encoder.
    assert!(joined.contains("-profile:v high"));
    assert!(joined.contains("-maxrate 8000k"));
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
