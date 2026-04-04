use std::{collections::HashMap, io::Read, time::Duration};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use flate2::read::GzDecoder;
use quick_xml::{Reader, events::Event, name::QName};
use reqwest::{Client, Url, header};
use tracing::warn;

const XMLTV_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);
const GZIP_MAGIC_BYTES: [u8; 2] = [0x1f, 0x8b];

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
}

pub async fn fetch_xmltv(client: &Client, url: &Url) -> Result<XmltvFeed> {
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
    };
    let body = response
        .bytes()
        .await
        .with_context(|| format!("unable to download XMLTV response body from {url}"))?;
    let decoded = decode_xmltv_bytes(&metadata, body.as_ref())?;
    parse_xmltv(&decoded)
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
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut channels = HashMap::new();
    let mut programmes = Vec::new();
    let mut current_programme: Option<PendingProgramme> = None;
    let mut current_channel: Option<PendingChannel> = None;
    let mut skipped_programmes = 0usize;

    loop {
        match reader.read_event() {
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
                if let Some(channel) = current_channel.as_mut() {
                    let display_name = reader
                        .read_text(QName(b"display-name"))
                        .context("unable to decode XMLTV display-name")?
                        .trim()
                        .to_string();
                    if !display_name.is_empty() {
                        channel.display_names.push(display_name);
                    }
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"channel" => {
                if let Some(channel) = current_channel.take() {
                    if channel.id.is_empty() {
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
                if let Some(programme) = current_programme.as_mut() {
                    programme.title = reader
                        .read_text(QName(b"title"))
                        .context("unable to decode XMLTV title")?
                        .trim()
                        .to_string();
                }
            }
            Ok(Event::Start(event)) if event.name().as_ref() == b"desc" => {
                if let Some(programme) = current_programme.as_mut() {
                    let description = reader
                        .read_text(QName(b"desc"))
                        .context("unable to decode XMLTV description")?
                        .trim()
                        .to_string();
                    programme.description = (!description.is_empty()).then_some(description);
                }
            }
            Ok(Event::End(event)) if event.name().as_ref() == b"programme" => {
                if let Some(programme) = current_programme.take() {
                    match finalize_programme(programme) {
                        Some(programme) => programmes.push(programme),
                        None => skipped_programmes += 1,
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => return Err(anyhow!("unable to parse XMLTV: {error}")),
        }
    }

    if skipped_programmes > 0 {
        warn!("skipped {skipped_programmes} malformed XMLTV programme entries");
    }

    Ok(XmltvFeed {
        channels,
        programmes,
    })
}

fn finalize_programme(programme: PendingProgramme) -> Option<XmltvProgramme> {
    if programme.channel_key.is_empty() || programme.title.is_empty() {
        return None;
    }

    let start_at = match parse_xmltv_timestamp(&programme.start_raw) {
        Ok(value) => value,
        Err(error) => {
            warn!(
                channel_key = programme.channel_key,
                start = programme.start_raw,
                "skipping XMLTV programme due to invalid start timestamp: {error}"
            );
            return None;
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
            return None;
        }
    };

    Some(XmltvProgramme {
        channel_key: programme.channel_key,
        title: programme.title,
        description: programme.description,
        start_at,
        end_at,
    })
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

    #[test]
    fn parses_xmltv_programmes_and_channels() {
        let xml = r#"
        <tv>
          <channel id="channel-1">
            <display-name>Arena 1 HD</display-name>
          </channel>
          <programme start="20260404120000 +0000" stop="20260404130000 +0000" channel="channel-1">
            <title>Lunch News</title>
            <desc>Midday headlines.</desc>
          </programme>
        </tv>
        "#;

        let feed = parse_xmltv(xml).expect("xml should parse");
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
        let xml = r#"
        <tv>
          <programme start="invalid" stop="20260404130000 +0000" channel="channel-1">
            <title>Broken row</title>
          </programme>
          <programme start="20260404140000 +0000" stop="20260404150000 +0000" channel="channel-2">
            <title>Working row</title>
          </programme>
        </tv>
        "#;

        let feed = parse_xmltv(xml).expect("xml should parse");
        assert_eq!(feed.programmes.len(), 1);
        assert_eq!(feed.programmes[0].channel_key, "channel-2");
        assert_eq!(feed.programmes[0].title, "Working row");
    }

    #[test]
    fn decodes_plain_xmltv_bytes() {
        let xml = "<tv></tv>";
        let decoded = decode_xmltv_bytes(
            &XmltvResponseMetadata {
                url: "https://example.com/feed.xml".to_string(),
                content_encoding: None,
                content_type: Some("application/xml".to_string()),
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
                content_type: Some("application/octet-stream".to_string()),
            },
            &gzip_bytes(xml),
        )
        .expect("gzip xml from url");

        assert_eq!(decoded, xml);
    }

    #[test]
    fn decodes_gzip_xmltv_bytes_from_content_encoding() {
        let xml = "<tv></tv>";
        let decoded = decode_xmltv_bytes(
            &XmltvResponseMetadata {
                url: "https://example.com/feed".to_string(),
                content_encoding: Some("gzip".to_string()),
                content_type: Some("application/xml".to_string()),
            },
            &gzip_bytes(xml),
        )
        .expect("gzip xml from header");

        assert_eq!(decoded, xml);
    }

    #[test]
    fn decodes_gzip_xmltv_bytes_from_magic_bytes() {
        let xml = "<tv></tv>";
        let decoded = decode_xmltv_bytes(
            &XmltvResponseMetadata {
                url: "https://example.com/feed".to_string(),
                content_encoding: None,
                content_type: Some("application/xml".to_string()),
            },
            &gzip_bytes(xml),
        )
        .expect("gzip xml from magic bytes");

        assert_eq!(decoded, xml);
    }
}
