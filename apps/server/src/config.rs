use std::{env, net::SocketAddr, str::FromStr};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use url::Url;

#[derive(Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
    pub relay_signing_secret: String,
    pub encryption_key: [u8; 32],
    pub access_token_minutes: i64,
    pub refresh_token_days: i64,
    pub relay_token_minutes: i64,
    pub daily_sync_hour_local: u32,
    pub public_origin: Option<Url>,
    pub allowed_origins: Vec<String>,
    pub browser_cookie_secure: bool,
    pub vpn_enabled: bool,
    pub vpn_provider_name: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_address = SocketAddr::from_str(&read_env("APP_BIND_ADDRESS")?)
            .context("APP_BIND_ADDRESS must be a valid socket address")?;
        let database_url = read_env("APP_DATABASE_URL")?;
        let jwt_secret = read_env("APP_JWT_SECRET")?;
        let relay_signing_secret =
            read_optional_env("APP_RELAY_SIGNING_SECRET")?.unwrap_or_else(|| jwt_secret.clone());
        let access_token_minutes = read_env("APP_ACCESS_TOKEN_MINUTES")?.parse()?;
        let refresh_token_days = read_env("APP_REFRESH_TOKEN_DAYS")?.parse()?;
        let relay_token_minutes = read_env_or_default("APP_RELAY_TOKEN_MINUTES", "120")?.parse()?;
        let daily_sync_hour_local =
            read_env_or_default("APP_DAILY_SYNC_HOUR_LOCAL", "6")?.parse()?;
        let public_origin = read_optional_env("APP_PUBLIC_ORIGIN")?
            .map(|value| Url::parse(&value).context("APP_PUBLIC_ORIGIN must be a valid URL"))
            .transpose()?;
        let mut allowed_origins = parse_allowed_origins(&read_env_or_default(
            "APP_ALLOWED_ORIGINS",
            "http://127.0.0.1:5173,http://localhost:5173,tauri://localhost",
        )?);
        if let Some(origin) = &public_origin {
            let origin = normalize_origin(origin.as_str());
            if !allowed_origins.iter().any(|value| value == &origin) {
                allowed_origins.push(origin);
            }
        }
        let browser_cookie_secure = read_optional_env("APP_BROWSER_COOKIE_SECURE")?
            .map(|value| parse_bool_env("APP_BROWSER_COOKIE_SECURE", &value))
            .transpose()?
            .unwrap_or_else(|| {
                public_origin
                    .as_ref()
                    .map(|origin| origin.scheme() == "https")
                    .unwrap_or(false)
            });
        let vpn_enabled = parse_bool_env(
            "APP_VPN_ENABLED",
            &read_env_or_default("APP_VPN_ENABLED", "false")?,
        )?;
        let vpn_provider_name = read_optional_env("APP_VPN_PROVIDER_NAME")?;
        let decoded_key = STANDARD
            .decode(read_env("APP_ENCRYPTION_KEY_B64")?)
            .context("APP_ENCRYPTION_KEY_B64 must be valid base64")?;
        let encryption_key: [u8; 32] = decoded_key.try_into().map_err(|_| {
            anyhow::anyhow!("APP_ENCRYPTION_KEY_B64 must decode to exactly 32 bytes")
        })?;

        Ok(Self {
            bind_address,
            database_url,
            jwt_secret,
            relay_signing_secret,
            encryption_key,
            access_token_minutes,
            refresh_token_days,
            relay_token_minutes,
            daily_sync_hour_local,
            public_origin,
            allowed_origins,
            browser_cookie_secure,
            vpn_enabled,
            vpn_provider_name,
        })
    }
}

fn read_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing environment variable {name}"))
}

fn read_env_or_default(name: &str, default_value: &str) -> Result<String> {
    Ok(env::var(name).unwrap_or_else(|_| default_value.to_string()))
}

fn read_optional_env(name: &str) -> Result<Option<String>> {
    match env::var(name) {
        Ok(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(trimmed.to_string()))
            }
        }
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to read environment variable {name}"))
        }
    }
}

fn parse_allowed_origins(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(normalize_origin)
        .collect()
}

fn normalize_origin(value: &str) -> String {
    value.trim_end_matches('/').to_string()
}

fn parse_bool_env(name: &str, value: &str) -> Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow::anyhow!("{name} must be a boolean value")),
    }
}
