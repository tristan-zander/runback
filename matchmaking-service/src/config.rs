use std::{error::Error, str::FromStr};

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tracing::Level;

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub enum LogLevel {
    TRACE,
    DEBUG,
    INFO,
    WARN,
    ERROR,
}

impl Into<Level> for LogLevel {
    fn into(self) -> Level {
        match self {
            Self::TRACE => Level::TRACE,
            Self::DEBUG => Level::DEBUG,
            Self::INFO => Level::INFO,
            Self::WARN => Level::WARN,
            Self::ERROR => Level::ERROR,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub rust_log: LogLevel,
    pub database_url: Url,
    pub redis_url: Option<Url>,
    pub kafka_url: Option<Url>,
    pub keycloak_realm: Url,
    pub client_id: String,
    pub client_secret: Option<String>,
    #[serde(skip)]
    pub raw: Option<Figment>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            rust_log: LogLevel::INFO,
            // It's fine to unwrap this because it shouldn't ever fail
            database_url: Url::from_str("postgresql://rematch:password@localhost/matchmaking")
                .unwrap(),
            redis_url: Default::default(),
            kafka_url: Default::default(),
            // It's fine to unwrap this because it shouldn't ever fail
            keycloak_realm: Url::from_str("http://localhost:8080/auth/realms/rematch").unwrap(),
            client_id: "matchmaking".into(),
            client_secret: None,
            raw: None,
        }
    }
}

impl Config {
    /// Load order:
    ///     1. Set default values
    ///     2. Config.toml (override)
    ///     3. Any environment variables (override)
    pub fn new() -> Result<Config, Box<dyn Error>> {
        let figment = Figment::from(Serialized::defaults(Config::default()))
            .merge(Toml::file("Config.toml"))
            .merge(Env::prefixed("MATCHMAKING_"));

        let mut config: Config = figment.extract()?;
        config.raw = Some(figment);

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // If this panics, then there's likely something wrong with the Urls in the config
    #[test]
    fn test_config_default() {
        let _config = Config::default();

        assert!(true);
    }
}
