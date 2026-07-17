use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use crate::xmltv::{self, XmltvFeed};

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
    pub hls_stream_origin: Option<String>,
    pub hls_stream_path: Option<String>,
}

#[derive(Debug, Clone)]
pub struct XtreamCategory {
    pub remote_category_id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct XtreamOnDemandTitle {
    pub remote_id: String,
    pub name: String,
    pub category_id: Option<String>,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub plot: Option<String>,
    pub genre: Option<String>,
    pub cast_names: Option<String>,
    pub director: Option<String>,
    pub release_date: Option<String>,
    pub rating: Option<f64>,
    pub duration_minutes: Option<i32>,
    pub container_extension: Option<String>,
    pub provider_updated_at: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct XtreamVodDetails {
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub plot: Option<String>,
    pub genre: Option<String>,
    pub cast_names: Option<String>,
    pub director: Option<String>,
    pub release_date: Option<String>,
    pub rating: Option<f64>,
    pub duration_minutes: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct XtreamEpisode {
    pub remote_id: String,
    pub season_number: i32,
    pub episode_number: i32,
    pub name: String,
    pub plot: Option<String>,
    pub duration_minutes: Option<i32>,
    pub poster_url: Option<String>,
    pub container_extension: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XtreamValidationPayload {
    user_info: Option<XtreamUserInfo>,
}

#[derive(Debug, Deserialize)]
struct XtreamUserInfo {
    auth: serde_json::Value,
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
    direct_source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XtreamVodInfoPayload {
    info: Option<XtreamVodInfoDetailsPayload>,
}

#[derive(Debug, Deserialize)]
struct XtreamVodInfoDetailsPayload {
    movie_image: Option<String>,
    backdrop_path: Option<serde_json::Value>,
    plot: Option<String>,
    genre: Option<String>,
    cast: Option<String>,
    director: Option<String>,
    #[serde(alias = "releaseDate", alias = "releasedate")]
    release_date: Option<String>,
    rating: Option<serde_json::Value>,
    duration_secs: Option<serde_json::Value>,
    duration: Option<String>,
}

#[derive(Debug, Deserialize)]
struct XtreamSeriesInfoPayload {
    #[serde(default)]
    episodes: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct XtreamEpisodePayload {
    id: serde_json::Value,
    episode_num: Option<serde_json::Value>,
    title: Option<String>,
    container_extension: Option<String>,
    info: Option<XtreamEpisodeInfoPayload>,
}

#[derive(Debug, Deserialize)]
struct XtreamEpisodeInfoPayload {
    plot: Option<String>,
    duration_secs: Option<serde_json::Value>,
    movie_image: Option<String>,
}

const BROWSER_HLS_PROBE_TIMEOUT: Duration = Duration::from_secs(8);

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

pub async fn fetch_on_demand_categories(
    client: &Client,
    credentials: &XtreamCredentials,
    media_type: &str,
) -> Result<Vec<XtreamCategory>> {
    let action = if media_type == "series" {
        "get_series_categories"
    } else {
        "get_vod_categories"
    };
    let url = player_api_url(credentials, Some(("action", action)))?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let items = json_collection_values(&payload)
        .context("Xtream on-demand categories response was not a collection")?;
    Ok(items
        .into_iter()
        .filter_map(|category| {
            let remote_category_id = object_string(category, "category_id")?;
            let name = object_string(category, "category_name")?;
            Some(XtreamCategory {
                remote_category_id,
                name,
            })
        })
        .collect())
}

pub async fn fetch_on_demand_titles(
    client: &Client,
    credentials: &XtreamCredentials,
    media_type: &str,
) -> Result<Vec<XtreamOnDemandTitle>> {
    let action = if media_type == "series" {
        "get_series"
    } else {
        "get_vod_streams"
    };
    let url = player_api_url(credentials, Some(("action", action)))?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;
    let items = json_collection_values(&payload)
        .context("Xtream on-demand titles response was not a collection")?;
    Ok(items
        .into_iter()
        .filter_map(|item| {
            let remote_id = object_string(
                item,
                if media_type == "series" {
                    "series_id"
                } else {
                    "stream_id"
                },
            )?;
            let name = object_string(item, "name")?;
            Some(XtreamOnDemandTitle {
                remote_id,
                name,
                category_id: object_string(item, "category_id"),
                poster_url: object_string(
                    item,
                    if media_type == "series" {
                        "cover"
                    } else {
                        "stream_icon"
                    },
                ),
                backdrop_url: item.get("backdrop_path").cloned().and_then(first_image_url),
                plot: object_string(item, "plot"),
                genre: object_string(item, "genre"),
                cast_names: object_string(item, "cast"),
                director: object_string(item, "director"),
                release_date: object_string(item, "release_date")
                    .or_else(|| object_string(item, "releaseDate"))
                    .or_else(|| object_string(item, "releasedate")),
                rating: value_to_f64(item.get("rating")),
                duration_minutes: if media_type == "series" {
                    item.get("episode_run_time").and_then(value_to_i32)
                } else {
                    value_to_i64(item.get("duration_secs")).map(|seconds| (seconds / 60) as i32)
                },
                container_extension: object_string(item, "container_extension"),
                provider_updated_at: value_to_i64(item.get(if media_type == "series" {
                    "last_modified"
                } else {
                    "added"
                })),
            })
        })
        .collect())
}

pub async fn fetch_vod_info(
    client: &Client,
    credentials: &XtreamCredentials,
    vod_id: &str,
) -> Result<XtreamVodDetails> {
    let url = player_api_url_with_params(
        credentials,
        &[("action", "get_vod_info"), ("vod_id", vod_id)],
    )?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<XtreamVodInfoPayload>()
        .await?;
    let info = payload
        .info
        .context("Xtream VOD info response did not contain info")?;
    Ok(XtreamVodDetails {
        poster_url: clean_optional(info.movie_image),
        backdrop_url: info.backdrop_path.and_then(first_image_url),
        plot: clean_optional(info.plot),
        genre: clean_optional(info.genre),
        cast_names: clean_optional(info.cast),
        director: clean_optional(info.director),
        release_date: clean_optional(info.release_date),
        rating: value_to_f64(info.rating.as_ref()),
        duration_minutes: value_to_i64(info.duration_secs.as_ref())
            .map(|seconds| (seconds / 60) as i32)
            .or_else(|| info.duration.as_deref().and_then(parse_duration_minutes)),
    })
}

pub async fn fetch_series_episodes(
    client: &Client,
    credentials: &XtreamCredentials,
    series_id: &str,
) -> Result<Vec<XtreamEpisode>> {
    let url = player_api_url_with_params(
        credentials,
        &[("action", "get_series_info"), ("series_id", series_id)],
    )?;
    let payload = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .json::<XtreamSeriesInfoPayload>()
        .await?;
    let mut episodes = Vec::new();
    let seasons = payload.episodes.as_object().cloned().unwrap_or_default();
    for (season_key, items) in seasons {
        let season_number = season_key.parse::<i32>().unwrap_or_default();
        let items = serde_json::from_value::<Vec<XtreamEpisodePayload>>(items)
            .with_context(|| format!("unable to decode Xtream season {season_key}"))?;
        for (index, item) in items.into_iter().enumerate() {
            let info = item.info;
            episodes.push(XtreamEpisode {
                remote_id: json_value_to_string(&item.id),
                season_number,
                episode_number: item
                    .episode_num
                    .as_ref()
                    .and_then(value_to_i32)
                    .unwrap_or(index as i32 + 1),
                name: item
                    .title
                    .unwrap_or_else(|| format!("Episode {}", index + 1)),
                plot: info.as_ref().and_then(|v| clean_optional(v.plot.clone())),
                duration_minutes: info
                    .as_ref()
                    .and_then(|v| value_to_i64(v.duration_secs.as_ref()))
                    .map(|seconds| (seconds / 60) as i32),
                poster_url: info.and_then(|v| clean_optional(v.movie_image)),
                container_extension: clean_optional(item.container_extension),
            });
        }
    }
    episodes.sort_by_key(|item| (item.season_number, item.episode_number));
    Ok(episodes)
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
        .map(|channel| {
            let (hls_stream_origin, hls_stream_path) = channel
                .direct_source
                .as_deref()
                .and_then(canonical_hls_stream_identity)
                .map(|(origin, path)| (Some(origin), Some(path)))
                .unwrap_or((None, None));
            XtreamChannel {
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
                hls_stream_origin,
                hls_stream_path,
            }
        })
        .collect())
}

pub fn canonical_hls_stream_identity(value: &str) -> Option<(String, String)> {
    let url = Url::parse(value.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return None;
    }
    let path = url.path();
    if path.is_empty() || path == "/" {
        return None;
    }
    let path_fingerprint = format!("sha256:{:x}", Sha256::digest(path.as_bytes()));
    Some((url.origin().ascii_serialization(), path_fingerprint))
}

pub async fn probe_hls_playlist_url(client: &Client, url: &str) -> Result<bool> {
    let response = client
        .get(url)
        .timeout(BROWSER_HLS_PROBE_TIMEOUT)
        .send()
        .await?;
    if !response.status().is_success() {
        return Ok(false);
    }

    let body = response.bytes().await?;
    let manifest = String::from_utf8_lossy(&body);
    Ok(looks_like_hls_playlist(&manifest))
}

pub async fn fetch_xmltv(client: &Client, credentials: &XtreamCredentials) -> Result<XmltvFeed> {
    let url = build_xmltv_url(credentials)?;
    xmltv::fetch_xmltv(client, &url).await
}

pub fn build_xmltv_url(credentials: &XtreamCredentials) -> Result<Url> {
    let mut url = normalized_base_url(&credentials.base_url)?;
    url.set_path("xmltv.php");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("username", &credentials.username);
        query.append_pair("password", &credentials.password);
    }

    Ok(url)
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

pub fn build_on_demand_stream_url(
    credentials: &XtreamCredentials,
    media_type: &str,
    stream_id: &str,
    extension: Option<&str>,
) -> Result<String> {
    let mut url = normalized_base_url(&credentials.base_url)?;
    let segment = if media_type == "series" {
        "series"
    } else {
        "movie"
    };
    let ext = extension.filter(|value| !value.is_empty()).unwrap_or("mp4");
    url.set_path(&format!(
        "{}/{}/{}/{}.{}",
        segment, credentials.username, credentials.password, stream_id, ext
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
    let params = action.map(|pair| vec![pair]).unwrap_or_default();
    player_api_url_with_params(credentials, &params)
}

fn player_api_url_with_params(
    credentials: &XtreamCredentials,
    params: &[(&str, &str)],
) -> Result<Url> {
    let mut url = normalized_base_url(&credentials.base_url)?;
    url.set_path("player_api.php");
    {
        let mut query = HashMap::new();
        query.insert("username", credentials.username.clone());
        query.insert("password", credentials.password.clone());
        for (key, value) in params {
            query.insert(*key, (*value).to_string());
        }
        let mut pairs = url.query_pairs_mut();
        for (key, value) in query {
            pairs.append_pair(key, &value);
        }
    }
    Ok(url)
}

fn looks_like_hls_playlist(body: &str) -> bool {
    let trimmed = body.trim_start();
    trimmed.starts_with("#EXTM3U")
        || trimmed.contains("\n#EXTM3U")
        || trimmed.contains("#EXT-X-")
        || trimmed.contains("#EXTINF:")
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

fn parse_duration_minutes(value: &str) -> Option<i32> {
    let parts = value
        .split(':')
        .map(|part| part.parse::<i32>().ok())
        .collect::<Option<Vec<_>>>()?;
    match parts.as_slice() {
        [hours, minutes, _seconds] => Some(hours.saturating_mul(60).saturating_add(*minutes)),
        [minutes, _seconds] => Some(*minutes),
        _ => None,
    }
}

fn first_image_url(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Array(values) => values
            .into_iter()
            .find_map(|value| value.as_str().map(str::to_string))
            .filter(|value| !value.is_empty()),
        serde_json::Value::String(value) if !value.is_empty() => Some(value),
        _ => None,
    }
}

fn json_collection_values(value: &serde_json::Value) -> Option<Vec<&serde_json::Value>> {
    if let Some(items) = value.as_array() {
        Some(items.iter().collect())
    } else {
        value.as_object().map(|items| items.values().collect())
    }
}

fn object_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .map(json_value_to_string)
        .filter(|value| !value.trim().is_empty() && value != "null")
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn value_to_i64(value: Option<&serde_json::Value>) -> Option<i64> {
    value.and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(string) => string.parse().ok(),
        _ => None,
    })
}

fn value_to_i32(value: &serde_json::Value) -> Option<i32> {
    value_to_i64(Some(value)).and_then(|value| i32::try_from(value).ok())
}

fn value_to_f64(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::String(string) => string.parse().ok(),
        _ => None,
    })
}

fn json_value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(string) => string.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(boolean) => boolean.to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn builds_on_demand_urls() {
        let credentials = XtreamCredentials {
            base_url: "https://iptv.example.com".to_string(),
            username: "user".to_string(),
            password: "pass".to_string(),
            output_format: "m3u8".to_string(),
        };

        assert!(
            build_on_demand_stream_url(&credentials, "movie", "42", Some("mp4"))
                .expect("movie url")
                .contains("movie/user/pass/42.mp4")
        );
        assert!(
            build_on_demand_stream_url(&credentials, "series", "84", Some("mkv"))
                .expect("episode url")
                .contains("series/user/pass/84.mkv")
        );
    }

    #[test]
    fn decodes_series_payload_with_string_ids() {
        let payload: serde_json::Value = serde_json::from_str(r#"{
          "series_id":"12","name":"Example","category_id":7,"rating":"8.5",
          "last_modified":"1710000000","episode_run_time":"45","backdrop_path":["https://img/back.jpg"]
        }"#).expect("series payload");
        assert_eq!(object_string(&payload, "series_id").as_deref(), Some("12"));
        assert_eq!(value_to_f64(payload.get("rating")), Some(8.5));
        assert_eq!(
            payload.get("episode_run_time").and_then(value_to_i32),
            Some(45)
        );
    }

    #[test]
    fn parses_provider_durations() {
        assert_eq!(parse_duration_minutes("01:42:30"), Some(102));
        assert_eq!(parse_duration_minutes("42:30"), Some(42));
        assert_eq!(parse_duration_minutes("unknown"), None);
    }

    #[test]
    fn canonicalizes_hls_identity_without_query_or_fragment() {
        assert_eq!(
            canonical_hls_stream_identity(
                "https://cdn.example.com/no-event/idle.m3u8?token=secret#x"
            ),
            Some((
                "https://cdn.example.com".to_string(),
                "sha256:f859a2cc2e26ebcc6d69014956aa9f71cd97383c42824d8fc3675569e2818121"
                    .to_string(),
            ))
        );
        assert_eq!(canonical_hls_stream_identity("file:///tmp/a.m3u8"), None);
    }

    #[test]
    fn detects_hls_playlists_from_manifest_markers() {
        assert!(looks_like_hls_playlist("#EXTM3U\n#EXT-X-VERSION:3\n"));
        assert!(looks_like_hls_playlist("#EXTINF:6,\nsegment001.ts\n"));
        assert!(!looks_like_hls_playlist("not a playlist"));
    }
}
