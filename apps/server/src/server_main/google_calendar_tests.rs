use super::*;

#[test]
fn calendar_url_encodes_identifiers_as_path_segments() {
    let url = google_event_url("calendar/user@example.com", Some("event id")).unwrap();
    assert!(
        url.as_str()
            .contains("calendar%2Fuser@example.com/events/event%20id")
    );
}
