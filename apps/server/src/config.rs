use std::{env, net::SocketAddr, str::FromStr};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};

#[derive(Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub jwt_secret: String,
    pub encryption_key: [u8; 32],
    pub access_token_minutes: i64,
    pub refresh_token_days: i64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bind_address = SocketAddr::from_str(&read_env("APP_BIND_ADDRESS")?)
            .context("APP_BIND_ADDRESS must be a valid socket address")?;
        let database_url = read_env("APP_DATABASE_URL")?;
        let jwt_secret = read_env("APP_JWT_SECRET")?;
        let access_token_minutes = read_env("APP_ACCESS_TOKEN_MINUTES")?.parse()?;
        let refresh_token_days = read_env("APP_REFRESH_TOKEN_DAYS")?.parse()?;
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
            encryption_key,
            access_token_minutes,
            refresh_token_days,
        })
    }
}

fn read_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("missing environment variable {name}"))
}
