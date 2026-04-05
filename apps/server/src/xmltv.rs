use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};
use async_compression::tokio::bufread::GzipDecoder as AsyncGzipDecoder;
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use futures_util::TryStreamExt;
use quick_xml::{Reader, events::Event};
use reqwest::{Client, Url, header};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio_util::io::StreamReader;
use tracing::{info, warn};

const XMLTV_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const GZIP_MAGIC_BYTES: [u8; 2] = [0x1f, 0x8b];
const XMLTV_PROGRAMME_RETENTION_PAST_HOURS: i64 = 2;
const XMLTV_PROGRAMME_RETENTION_FUTURE_HOURS: i64 = 6;

#[derive(Debug, Clone)]
pub struct XmltvChannel {
    pub id: String,
    pub display_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct XmltvProgramme {
    pub channel_key: String,
    pub title: String,
    pub description: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default)]
pub struct XmltvFeed {
    pub channels: HashMap<String, XmltvChannel>,
    pub programmes: Vec<XmltvProgramme>,
}

#[derive(Debug, Clone, Default)]
struct PendingProgramme {
    channel_key: String,
    start_raw: String,
    end_raw: String,
    title: String,
    description: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct PendingChannel {
    id: String,
    display_names: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct XmltvResponseMetadata {
    pub url: String,
    pub content_encoding: Option<String>,
    pub content_type: Option<String>,
    pub content_length: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextTarget {
    ChannelDisplayName,
    ProgrammeTitle,
    ProgrammeDescription,
}

#[derive(Debug, Clone)]
enum FinalizeProgrammeOutcome {
    Accepted(XmltvProgramme),
    Malformed,
    OutOfWindow,
}

pub async fn fetch_xmltv(client: &Client, url: &Url) -> Result<XmltvFeed> {
    let started_at = Instant::now();
    let response = client
        .get(url.clone())
        .timeout(XMLTV_REQUEST_TIMEOUT)
        .send()
        .await
        .with_context(|| format!("unable to request XMLTV feed from {url}"))?
        .error_for_status()
        .with_context(|| format!("provider rejected XMLTV request for {url}"))?;
    let metadata = XmltvResponseMetadata {
        url: url.to_string(),
        content_encoding: response
            .headers()
            .get(header::CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        content_type: response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        content_length: response.content_length(),
    };
    let downloaded_bytes = Arc::new(AtomicU64::new(0));
    let tracked_downloaded_bytes = Arc::clone(&downloaded_bytes);
    let stream = response
        .bytes_stream()
        .inspect_ok(move |chunk| {
            tracked_downloaded_bytes.fetch_add(chunk.len() as u64, Ordering::Relaxed);
        })
        .map_err(std::io::Error::other);
    let reader = StreamReader::new(stream);
    let feed = parse_xmltv_stream(&metadata, reader).await?;
    let elapsed_ms = started_at.elapsed().as_millis() as u64;

    info!(
        url = %metadata.url,
        downloaded_bytes = downloaded_bytes.load(Ordering::Relaxed),
        advertised_content_length = metadata.content_length,
        programme_count = feed.programmes.len(),
        channel_count = feed.channels.len(),
        elapsed_ms,
        "fetched and parsed XMLTV feed"
    );

    Ok(feed)
}

pub fn decode_xmltv_bytes(metadata: &XmltvResponseMetadata, bytes: &[u8]) -> Result<String> {
    let decoded_bytes = if should_decompress_gzip(metadata, bytes) {
        let mut decoder = GzDecoder::new(bytes);
        let mut buffer = Vec::new();
        decoder
            .read_to_end(&mut buffer)
            .context("unable to decompress gzip XMLTV feed")?;
        buffer
    } else {
        bytes.to_vec()
    };

    String::from_utf8(decoded_bytes)
        .or_else(|error| Ok(String::from_utf8_lossy(&error.into_bytes()).into_owned()))
}

async fn parse_xmltv_stream<R>(metadata: &XmltvResponseMetadata, reader: R) -> Result<XmltvFeed>
where
    R: AsyncBufRead + Unpin,
{
    let mut reader = AsyncBufReader::new(reader);
    let peeked_bytes = reader
        .fill_buf()
        .await
        .context("unable to inspect XMLTV stream header")?;
    let should_decompress = should_decompress_gzip(metadata, peeked_bytes);
    let parse_started_at = Instant::now();
    let feed = if should_decompress {
        let decoder = AsyncGzipDecoder::new(reader);
        parse_xmltv_async(AsyncBufReader::new(decoder)).await?
    } else {
        parse_xmltv_async(reader).await?
    };

    info!(
        url = %metadata.url,
        compressed = should_decompress,
        programme_count = feed.programmes.len(),
        channel_count = feed.channels.len(),
        parse_elapsed_ms = parse_started_at.elapsed().as_millis() as u64,
        "parsed XMLTV stream"
    );

    Ok(feed)
}

fn should_decompress_gzip(metadata: &XmltvResponseMetadata, bytes: &[u8]) -> bool {
    let content_encoding = metadata
        .content_encoding
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let content_type = metadata
        .content_type
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let url = metadata.url.to_ascii_lowercase();

    content_encoding.contains("gzip")
        || content_type.contains("gzip")
        || url.ends_with(".gz")
        || bytes.starts_with(&GZIP_MAGIC_BYTES)
}

pub fn parse_xmltv(xml: &str) -> Result<XmltvFeed> {
    let reader = Reader::from_reader(BufReader::new(xml.as_bytes()));
    parse_xmltv_reader(reader)
}

fn parse_xmltv_reader<R>(mut reader: Reader<R>) -> Result<XmltvFeed>
where
    R: BufRead,
{
    reader.config_mut().trim_text(true);
    let mut channels = HashMap::new();
    let mut programmes = Vec::new();
    let mut current_programme: Option<PendingProgramme> = None;
    let mut current_channel: Option<PendingChannel> = None;
    let mut skipped_malformed_programmes = 0usize;
    let mut skipped_out_of_window_programmes = 0usize;
    let mut text_target: Option<TextTarget> = None;
    let mut text_buffer = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(event)) if event.name().as_ref() == b"channel" => {
                let mut pending = PendingChannel::default();
                for attribute in event.attributes().with_checks(false).flatten() {
                    if attribute.key.as_ref() == b"id" {
                        pending.id = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
                    }
                }
                current_channel = Some(pending);
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"display-name" => {
                text_target = current_channel
                    .as_ref()
                    .map(|_| TextTarget::ChannelDisplayName);
                text_buffer.clear();
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"channel" => {
                if let Some(channel) = current_channel.take() {
                    if channel.id.is_empty() {
                        buf.clear();
                        continue;
                    }

                    channels
                        .entry(channel.id.clone())
                        .and_modify(|existing: &mut XmltvChannel| {
                            for display_name in &channel.display_names {
                                if !existing.display_names.contains(display_name) {
                                    existing.display_names.push(display_name.clone());
                                }
                            }
                        })
                        .or_insert(XmltvChannel {
                            id: channel.id,
                            display_names: channel.display_names,
                        });
                }
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"programme" => {
                let mut pending = PendingProgramme::default();
                for attribute in event.attributes().with_checks(false).flatten() {
                    let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
                    match attribute.key.as_ref() {
                        b"channel" => pending.channel_key = value,
                        b"start" => pending.start_raw = value,
                        b"stop" => pending.end_raw = value,
                        _ => {}
                    }
                }
                current_programme = Some(pending);
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"title" => {
                text_target = current_programme
                    .as_ref()
                    .map(|_| TextTarget::ProgrammeTitle);
                text_buffer.clear();
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"desc" => {
                text_target = current_programme
                    .as_ref()
                    .map(|_| TextTarget::ProgrammeDescription);
                text_buffer.clear();
            }
            Ok(Event::Text(event)) => {
                if text_target.is_some() {
                    text_buffer
                        .push_str(&event.decode().context("unable to decode XMLTV text node")?);
                }
            }
            Ok(Event::CData(event)) => {
                if text_target.is_some() {
                    text_buffer.push_str(
                        &event
                            .decode()
                            .context("unable to decode XMLTV CDATA node")?,
                    );
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"display-name" => {
                if matches!(text_target, Some(TextTarget::ChannelDisplayName)) {
                    if let Some(channel) = current_channel.as_mut() {
                        let display_name = text_buffer.trim().to_string();
                        if !display_name.is_empty() {
                            channel.display_names.push(display_name);
                        }
                    }
                    text_target = None;
                    text_buffer.clear();
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"title" => {
                if matches!(text_target, Some(TextTarget::ProgrammeTitle)) {
                    if let Some(programme) = current_programme.as_mut() {
                        programme.title = text_buffer.trim().to_string();
                    }
                    text_target = None;
                    text_buffer.clear();
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"desc" => {
                if matches!(text_target, Some(TextTarget::ProgrammeDescription)) {
                    if let Some(programme) = current_programme.as_mut() {
                        let description = text_buffer.trim().to_string();
                        programme.description = (!description.is_empty()).then_some(description);
                    }
                    text_target = None;
                    text_buffer.clear();
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"programme" => {
                if let Some(programme) = current_programme.take() {
                    match finalize_programme(programme, Utc::now()) {
                        FinalizeProgrammeOutcome::Accepted(programme) => programmes.push(programme),
                        FinalizeProgrammeOutcome::Malformed => skipped_malformed_programmes += 1,
                        FinalizeProgrammeOutcome::OutOfWindow => {
                            skipped_out_of_window_programmes += 1
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow!("unable to parse XMLTV: {error}")),
        }

        buf.clear();
    }

    if skipped_malformed_programmes > 0 {
        warn!("skipped {skipped_malformed_programmes} malformed XMLTV programme entries");
    }
    if skipped_out_of_window_programmes > 0 {
        info!(
            skipped_out_of_window_programmes,
            retention_past_hours = XMLTV_PROGRAMME_RETENTION_PAST_HOURS,
            retention_future_hours = XMLTV_PROGRAMME_RETENTION_FUTURE_HOURS,
            "discarded XMLTV programme entries outside retention window"
        );
    }

    Ok(XmltvFeed {
        channels,
        programmes,
    })
}

async fn parse_xmltv_async<R>(reader: R) -> Result<XmltvFeed>
where
    R: AsyncBufRead + Unpin,
{
    let mut reader = Reader::from_reader(reader);
    reader.config_mut().trim_text(true);
    let mut channels = HashMap::new();
    let mut programmes = Vec::new();
    let mut current_programme: Option<PendingProgramme> = None;
    let mut current_channel: Option<PendingChannel> = None;
    let mut skipped_malformed_programmes = 0usize;
    let mut skipped_out_of_window_programmes = 0usize;
    let mut text_target: Option<TextTarget> = None;
    let mut text_buffer = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into_async(&mut buf).await {
            Ok(Event::Start(event)) if event.name().as_ref() == b"channel" => {
                let mut pending = PendingChannel::default();
                for attribute in event.attributes().with_checks(false).flatten() {
                    if attribute.key.as_ref() == b"id" {
                        pending.id = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
                    }
                }
                current_channel = Some(pending);
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"display-name" => {
                text_target = current_channel
                    .as_ref()
                    .map(|_| TextTarget::ChannelDisplayName);
                text_buffer.clear();
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"channel" => {
                if let Some(channel) = current_channel.take() {
                    if channel.id.is_empty() {
                        buf.clear();
                        continue;
                    }

                    channels
                        .entry(channel.id.clone())
                        .and_modify(|existing: &mut XmltvChannel| {
                            for display_name in &channel.display_names {
                                if !existing.display_names.contains(display_name) {
                                    existing.display_names.push(display_name.clone());
                                }
                            }
                        })
                        .or_insert(XmltvChannel {
                            id: channel.id,
                            display_names: channel.display_names,
                        });
                }
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"programme" => {
                let mut pending = PendingProgramme::default();
                for attribute in event.attributes().with_checks(false).flatten() {
                    let value = String::from_utf8_lossy(attribute.value.as_ref()).into_owned();
                    match attribute.key.as_ref() {
                        b"channel" => pending.channel_key = value,
                        b"start" => pending.start_raw = value,
                        b"stop" => pending.end_raw = value,
                        _ => {}
                    }
                }
                current_programme = Some(pending);
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"title" => {
                text_target = current_programme
                    .as_ref()
                    .map(|_| TextTarget::ProgrammeTitle);
                text_buffer.clear();
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"desc" => {
                text_target = current_programme
                    .as_ref()
                    .map(|_| TextTarget::ProgrammeDescription);
                text_buffer.clear();
            }
            Ok(Event::Text(event)) => {
                if text_target.is_some() {
                    text_buffer
                        .push_str(&event.decode().context("unable to decode XMLTV text node")?);
                }
            }
            Ok(Event::CData(event)) => {
                if text_target.is_some() {
                    text_buffer.push_str(
                        &event
                            .decode()
                            .context("unable to decode XMLTV CDATA node")?,
                    );
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"display-name" => {
                if matches!(text_target, Some(TextTarget::ChannelDisplayName)) {
                    if let Some(channel) = current_channel.as_mut() {
                        let display_name = text_buffer.trim().to_string();
                        if !display_name.is_empty() {
                            channel.display_names.push(display_name);
                        }
                    }
                    text_target = None;
                    text_buffer.clear();
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"title" => {
                if matches!(text_target, Some(TextTarget::ProgrammeTitle)) {
                    if let Some(programme) = current_programme.as_mut() {
                        programme.title = text_buffer.trim().to_string();
                    }
                    text_target = None;
                    text_buffer.clear();
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"desc" => {
                if matches!(text_target, Some(TextTarget::ProgrammeDescription)) {
                    if let Some(programme) = current_programme.as_mut() {
                        let description = text_buffer.trim().to_string();
                        programme.description = (!description.is_empty()).then_some(description);
                    }
                    text_target = None;
                    text_buffer.clear();
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"programme" => {
                if let Some(programme) = current_programme.take() {
                    match finalize_programme(programme, Utc::now()) {
                        FinalizeProgrammeOutcome::Accepted(programme) => programmes.push(programme),
                        FinalizeProgrammeOutcome::Malformed => skipped_malformed_programmes += 1,
                        FinalizeProgrammeOutcome::OutOfWindow => {
                            skipped_out_of_window_programmes += 1
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow!("unable to parse XMLTV: {error}")),
        }

        buf.clear();
    }

    if skipped_malformed_programmes > 0 {
        warn!("skipped {skipped_malformed_programmes} malformed XMLTV programme entries");
    }
    if skipped_out_of_window_programmes > 0 {
        info!(
            skipped_out_of_window_programmes,
            retention_past_hours = XMLTV_PROGRAMME_RETENTION_PAST_HOURS,
            retention_future_hours = XMLTV_PROGRAMME_RETENTION_FUTURE_HOURS,
            "discarded XMLTV programme entries outside retention window"
        );
    }

    Ok(XmltvFeed {
        channels,
        programmes,
    })
}

fn finalize_programme(programme: PendingProgramme, now: DateTime<Utc>) -> FinalizeProgrammeOutcome {
    if programme.channel_key.is_empty() || programme.title.is_empty() {
        return FinalizeProgrammeOutcome::Malformed;
    }

    let start_at = match parse_xmltv_timestamp(&programme.start_raw) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                channel_key = programme.channel_key,
                start = programme.start_raw,
                "skipping XMLTV programme due to invalid start timestamp: {error}"
            );
            return FinalizeProgrammeOutcome::Malformed;
        }
    };

    let end_at = match parse_xmltv_timestamp(&programme.end_raw) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                channel_key = programme.channel_key,
                stop = programme.end_raw,
                "skipping XMLTV programme due to invalid stop timestamp: {error}"
            );
            return FinalizeProgrammeOutcome::Malformed;
        }
    };

    if !programme_is_within_retention_window(start_at, end_at, now) {
        return FinalizeProgrammeOutcome::OutOfWindow;
    }

    FinalizeProgrammeOutcome::Accepted(XmltvProgramme {
        channel_key: programme.channel_key,
        title: programme.title,
        description: programme.description,
        start_at,
        end_at,
    })
}

fn programme_is_within_retention_window(
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
    now: DateTime<Utc>,
) -> bool {
    let earliest_end = now - chrono::Duration::hours(XMLTV_PROGRAMME_RETENTION_PAST_HOURS);
    let latest_start = now + chrono::Duration::hours(XMLTV_PROGRAMME_RETENTION_FUTURE_HOURS);
    end_at > earliest_end && start_at < latest_start
}

fn parse_xmltv_timestamp(value: &str) -> Result<DateTime<Utc>> {
    if value.len() < 14 {
        return Err(anyhow!("invalid XMLTV timestamp {value}"));
    }

    let normalized = if value.contains(' ') {
        value.to_string()
    } else {
        format!("{} +0000", &value[..14])
    };

    let parsed = DateTime::parse_from_str(&normalized, "%Y%m%d%H%M%S %z")
        .or_else(|_| DateTime::parse_from_str(&normalized, "%Y%m%d%H%M %z"))?;
    Ok(parsed.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
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
            start_raw: "20260405180000 +0000".to_string(),
            end_raw: "20260405190000 +0000".to_string(),
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
            start_raw: "20260405175959 +0000".to_string(),
            end_raw: "20260405190000 +0000".to_string(),
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
}
