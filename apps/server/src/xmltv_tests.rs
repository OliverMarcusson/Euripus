use flate2::{Compression, write::GzEncoder};

use super::*;

fn gzip_bytes(input: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    std::io::Write::write_all(&mut encoder, input.as_bytes()).expect("gzip write");
    encoder.finish().expect("gzip finish")
}

fn xmltv_timestamp(value: DateTime<Utc>) -> String {
    value.format("%Y%m%d%H%M%S +0000").to_string()
}

#[test]
fn parses_xmltv_programmes_and_channels() {
    let start_at = Utc::now() - chrono::Duration::minutes(15);
    let end_at = start_at + chrono::Duration::hours(1);
    let xml = format!(
        r#"
        <tv>
          <channel id="channel-1">
            <display-name>Arena 1 HD</display-name>
          </channel>
          <programme start="{}" stop="{}" channel="channel-1">
            <title>Lunch News</title>
            <desc>Midday headlines.</desc>
          </programme>
        </tv>
        "#,
        xmltv_timestamp(start_at),
        xmltv_timestamp(end_at)
    );

    let feed = parse_xmltv(&xml).expect("xml should parse");
    assert_eq!(feed.programmes.len(), 1);
    assert_eq!(feed.programmes[0].title, "Lunch News");
    assert_eq!(
        feed.channels
            .get("channel-1")
            .map(|channel| channel.display_names.clone()),
        Some(vec!["Arena 1 HD".to_string()])
    );
}

#[test]
fn skips_invalid_xmltv_programmes_without_failing_the_feed() {
    let working_start_at = Utc::now() + chrono::Duration::minutes(30);
    let working_end_at = working_start_at + chrono::Duration::hours(1);
    let xml = format!(
        r#"
        <tv>
          <programme start="invalid" stop="20260404130000 +0000" channel="channel-1">
            <title>Broken row</title>
          </programme>
          <programme start="{}" stop="{}" channel="channel-2">
            <title>Working row</title>
          </programme>
        </tv>
        "#,
        xmltv_timestamp(working_start_at),
        xmltv_timestamp(working_end_at)
    );

    let feed = parse_xmltv(&xml).expect("xml should parse");
    assert_eq!(feed.programmes.len(), 1);
    assert_eq!(feed.programmes[0].channel_key, "channel-2");
    assert_eq!(feed.programmes[0].title, "Working row");
}

#[test]
fn discards_programmes_outside_retention_window() {
    let now = DateTime::parse_from_rfc3339("2026-04-05T12:00:00Z")
        .expect("fixed now")
        .with_timezone(&Utc);

    let old_programme = PendingProgramme {
        channel_key: "channel-1".to_string(),
        start_raw: "20260405070000 +0000".to_string(),
        end_raw: "20260405095959 +0000".to_string(),
        title: "Old".to_string(),
        description: None,
    };
    let future_programme = PendingProgramme {
        channel_key: "channel-1".to_string(),
        start_raw: "20260412120000 +0000".to_string(),
        end_raw: "20260412130000 +0000".to_string(),
        title: "Future".to_string(),
        description: None,
    };

    assert!(matches!(
        finalize_programme(old_programme, now),
        FinalizeProgrammeOutcome::OutOfWindow
    ));
    assert!(matches!(
        finalize_programme(future_programme, now),
        FinalizeProgrammeOutcome::OutOfWindow
    ));
}

#[test]
fn keeps_programmes_inside_retention_window() {
    let now = DateTime::parse_from_rfc3339("2026-04-05T12:00:00Z")
        .expect("fixed now")
        .with_timezone(&Utc);

    let recent_programme = PendingProgramme {
        channel_key: "channel-1".to_string(),
        start_raw: "20260405093000 +0000".to_string(),
        end_raw: "20260405100500 +0000".to_string(),
        title: "Recent".to_string(),
        description: None,
    };
    let upcoming_programme = PendingProgramme {
        channel_key: "channel-1".to_string(),
        start_raw: "20260412115959 +0000".to_string(),
        end_raw: "20260412130000 +0000".to_string(),
        title: "Upcoming".to_string(),
        description: None,
    };

    assert!(matches!(
        finalize_programme(recent_programme, now),
        FinalizeProgrammeOutcome::Accepted(_)
    ));
    assert!(matches!(
        finalize_programme(upcoming_programme, now),
        FinalizeProgrammeOutcome::Accepted(_)
    ));
}

#[test]
fn parses_compact_xmltv_timestamps_with_timezone_offsets() {
    let parsed = parse_xmltv_timestamp("20260405120000+0200").expect("compact timestamp");

    assert_eq!(
        parsed,
        DateTime::parse_from_rfc3339("2026-04-05T10:00:00Z")
            .expect("expected utc")
            .with_timezone(&Utc)
    );
}

#[test]
fn decodes_plain_xmltv_bytes() {
    let xml = "<tv></tv>";
    let decoded = decode_xmltv_bytes(
        &XmltvResponseMetadata {
            url: "https://example.com/feed.xml".to_string(),
            content_encoding: None,
            content_type: Some("application/xml".to_string()),
            content_length: Some(xml.len() as u64),
        },
        xml.as_bytes(),
    )
    .expect("plain xml");

    assert_eq!(decoded, xml);
}

#[test]
fn decodes_gzip_xmltv_bytes_from_gz_url() {
    let xml = "<tv></tv>";
    let decoded = decode_xmltv_bytes(
        &XmltvResponseMetadata {
            url: "https://example.com/feed.xml.gz".to_string(),
            content_encoding: None,
            content_type: Some("application/xml".to_string()),
            content_length: None,
        },
        &gzip_bytes(xml),
    )
    .expect("gzip xml");

    assert_eq!(decoded, xml);
}

#[test]
fn decodes_gzip_xmltv_bytes_from_content_encoding() {
    let xml = "<tv></tv>";
    let decoded = decode_xmltv_bytes(
        &XmltvResponseMetadata {
            url: "https://example.com/feed.xml".to_string(),
            content_encoding: Some("gzip".to_string()),
            content_type: Some("application/octet-stream".to_string()),
            content_length: None,
        },
        &gzip_bytes(xml),
    )
    .expect("gzip xml");

    assert_eq!(decoded, xml);
}

#[test]
fn decodes_gzip_xmltv_bytes_from_magic_bytes() {
    let xml = "<tv></tv>";
    let bytes = gzip_bytes(xml);
    let decoded = decode_xmltv_bytes(
        &XmltvResponseMetadata {
            url: "https://example.com/feed.bin".to_string(),
            content_encoding: None,
            content_type: Some("application/octet-stream".to_string()),
            content_length: None,
        },
        &bytes,
    )
    .expect("gzip xml");

    assert_eq!(decoded, xml);
}

#[test]
fn tolerates_non_utf8_xmltv_bytes() {
    let decoded = decode_xmltv_bytes(
        &XmltvResponseMetadata {
            url: "https://example.com/feed.xml".to_string(),
            content_encoding: None,
            content_type: Some("application/xml".to_string()),
            content_length: None,
        },
        &[0x66, 0x6f, 0x80, 0x6f],
    )
    .expect("lossy decode");

    assert_eq!(decoded, "fo\u{fffd}o");
}
