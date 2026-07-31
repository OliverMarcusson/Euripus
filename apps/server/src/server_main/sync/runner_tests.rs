use super::*;

#[test]
fn channel_sync_jobs_refresh_channels_without_epg() {
    assert!(should_refresh_channels("channels", 5));
    assert!(!should_sync_epg("channels"));
    assert!(should_refresh_channels("epg", 0));
    assert!(should_sync_epg("epg"));
}
