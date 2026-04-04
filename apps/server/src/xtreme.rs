use std::{collections::HashMap, time::Duration};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use quick_xml::{Reader, events::Event, name::QName};
use reqwest::Client;
use serde::Deserialize;
use tracing::warn;
use url::Url;

const XMLTV_REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, Clone)]
pub struct XtreamCredentials {
    pub base_url: String,
    pub username: String,
    pub password: String,
    pub output_format: String,
}

#[derive(Debug, Clone)]
pub struct XtreamValidation {
    pub valid: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct XtreamChannel {
    pub remote_stream_id: i32,
    pub name: String,
    pub logo_url: Option<String>,
    pub category_id: Option<String>,
    pub epg_channel_id: Option<String>,
    pub has_catchup: bool,
    pub archive_duration_hours: Option<i32>,
    pub stream_extension: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XtreamCategory {
    pub remote_category_id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct XtreamProgramme {
    pub channel_key: String,
    pub title: String,
    pub description: Option<String>,
    pub start_at: DateTime<Utc>,
    pub end_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
struct PendingProgramme {
    channel_key: String,
    start_raw: String,
    end_raw: String,
    title: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XtreamValidationPayload {
    user_info: Option<XtreamUserInfo>,
}

#[derive(Debug, Deserialize)]
struct XtreamUserInfo {
    auth: serde_json::Value,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XtreamCategoryPayload {
    category_id: serde_json::Value,
    category_name: String,
}

#[derive(Debug, Deserialize)]
struct XtreamChannelPayload {
    stream_id: i32,
    name: String,
    stream_icon: Option<String>,
    category_id: Option<serde_json::Value>,
    epg_channel_id: Option<String>,
    tv_archive: Option<serde_json::Value>,
    tv_archive_duration: Option<serde_json::Value>,
    container_extension: Option<String>,
}

pub async fn validate_profile(
    client: &Client,
    credentials: &XtreamCredentials,
) -> Result<XtreamValidation> {
    let url = player_api_url(credentials, None)?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<XtreamValidationPayload>()
        .await
        .context("unable to decode Xtreme validation response")?;

    let valid = payload
        .user_info
        .map(|user_info| xtream_truthy(&user_info.auth))
        .unwrap_or(false);

    let message = if valid {
        "Provider credentials validated".to_string()
    } else {
        "Provider rejected the supplied credentials".to_string()
    };

    Ok(XtreamValidation { valid, message })
}

pub async fn fetch_categories(
    client: &Client,
    credentials: &XtreamCredentials,
) -> Result<Vec<XtreamCategory>> {
    let url = player_api_url(credentials, Some(("action", "get_live_categories")))?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<XtreamCategoryPayload>>()
        .await?;

    Ok(payload
        .into_iter()
        .map(|category| XtreamCategory {
            remote_category_id: json_value_to_string(&category.category_id),
            name: category.category_name,
        })
        .collect())
}

pub async fn fetch_live_streams(
    client: &Client,
    credentials: &XtreamCredentials,
) -> Result<Vec<XtreamChannel>> {
    let url = player_api_url(credentials, Some(("action", "get_live_streams")))?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<Vec<XtreamChannelPayload>>()
        .await?;

    Ok(payload
        .into_iter()
        .map(|channel| XtreamChannel {
            remote_stream_id: channel.stream_id,
            name: channel.name,
            logo_url: channel.stream_icon,
            category_id: channel
                .category_id
                .map(|value| json_value_to_string(&value)),
            epg_channel_id: channel.epg_channel_id.filter(|value| !value.is_empty()),
            has_catchup: channel
                .tv_archive
                .as_ref()
                .map(xtream_truthy)
                .unwrap_or(false),
            archive_duration_hours: channel
                .tv_archive_duration
                .as_ref()
                .map(json_value_to_string)
                .and_then(|value| value.parse::<i32>().ok()),
            stream_extension: channel.container_extension,
        })
        .collect())
}

pub async fn fetch_xmltv(
    client: &Client,
    credentials: &XtreamCredentials,
) -> Result<Vec<XtreamProgramme>> {
    let mut url = normalized_base_url(&credentials.base_url)?;
    url.set_path("xmltv.php");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("username", &credentials.username);
        query.append_pair("password", &credentials.password);
    }

    let body = client
        .get(url)
        .timeout(XMLTV_REQUEST_TIMEOUT)
        .send()
        .await
        .context("unable to request XMLTV feed")?
        .error_for_status()
        .context("provider rejected XMLTV request")?
        .text()
        .await
        .context("unable to download XMLTV response body")?;
    parse_xmltv(&body)
}

pub fn build_live_stream_url(
    credentials: &XtreamCredentials,
    stream_id: i32,
    extension: Option<&str>,
) -> Result<String> {
    let mut url = normalized_base_url(&credentials.base_url)?;
    let ext = extension.unwrap_or(&credentials.output_format);
    url.set_path(&format!(
        "live/{}/{}/{}.{}",
        credentials.username, credentials.password, stream_id, ext
    ));
    Ok(url.to_string())
}

pub fn build_catchup_url(
    credentials: &XtreamCredentials,
    stream_id: i32,
    extension: Option<&str>,
    start_at: DateTime<Utc>,
    end_at: DateTime<Utc>,
) -> Result<String> {
    let duration_minutes = (end_at - start_at).num_minutes().max(1);
    let start_value = start_at.format("%Y-%m-%d:%H-%M").to_string();
    let ext = extension.unwrap_or(&credentials.output_format);
    let mut url = normalized_base_url(&credentials.base_url)?;
    url.set_path(&format!(
        "timeshift/{}/{}/{}/{}/{}.{}",
        credentials.username, credentials.password, duration_minutes, start_value, stream_id, ext
    ));
    Ok(url.to_string())
}

fn player_api_url(credentials: &XtreamCredentials, action: Option<(&str, &str)>) -> Result<Url> {
    let mut url = normalized_base_url(&credentials.base_url)?;
    url.set_path("player_api.php");
    {
        let mut query = HashMap::new();
        query.insert("username", credentials.username.clone());
        query.insert("password", credentials.password.clone());
        if let Some((key, value)) = action {
            query.insert(key, value.to_string());
        }
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, &value);
        }
    }
    Ok(url)
}

fn normalized_base_url(raw: &str) -> Result<Url> {
    let mut url = Url::parse(raw).with_context(|| format!("invalid base url {raw}"))?;
    if url.path() == "/" {
        url.set_path("");
    }
    Ok(url)
}

fn xtream_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Bool(boolean) => *boolean,
        serde_json::Value::Number(number) => number.as_i64().unwrap_or_default() > 0,
        serde_json::Value::String(string) => matches!(string.as_str(), "1" | "true" | "True"),
        _ => false,
    }
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(string) => string.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(boolean) => boolean.to_string(),
        _ => String::new(),
    }
}

fn parse_xmltv(xml: &str) -> Result<Vec<XtreamProgramme>> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut programmes = Vec::new();
    let mut current_programme: Option<PendingProgramme> = None;
    let mut skipped_programmes = 0usize;

    loop {
        match reader.read_event() {
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

    Ok(programmes)
}

fn finalize_programme(programme: PendingProgramme) -> Option<XtreamProgramme> {
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

    Some(XtreamProgramme {
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
    use super::*;

    #[test]
    fn parses_xmltv_programmes() {
        let xml = r#"
        <tv>
          <programme start="20260404120000 +0000" stop="20260404130000 +0000" channel="channel-1">
            <title>Lunch News</title>
            <desc>Midday headlines.</desc>
          </programme>
        </tv>
        "#;

        let programmes = parse_xmltv(xml).expect("xml should parse");
        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].title, "Lunch News");
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

        let programmes = parse_xmltv(xml).expect("xml should parse");
        assert_eq!(programmes.len(), 1);
        assert_eq!(programmes[0].channel_key, "channel-2");
        assert_eq!(programmes[0].title, "Working row");
    }

    #[test]
    fn builds_catchup_urls() {
        let credentials = XtreamCredentials {
            base_url: "https://iptv.example.com".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            output_format: "m3u8".to_string(),
        };

        let url = build_catchup_url(
            &credentials,
            7,
            Some("m3u8"),
            DateTime::parse_from_rfc3339("2026-04-04T12:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
            DateTime::parse_from_rfc3339("2026-04-04T13:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        )
        .expect("catchup url");

        assert!(url.contains("timeshift/user/pass/60/2026-04-04:12-00/7.m3u8"));
    }
}
