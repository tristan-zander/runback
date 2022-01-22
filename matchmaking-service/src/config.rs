use std::{error::Error, str::FromStr};

use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment, Profile,
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
    pub storage: Storage,
    pub events: Events,
    pub auth: Auth,

    #[serde(skip)]
    pub raw: Option<Figment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Storage {
    pub database_url: Url,
    pub redis_url: Option<Url>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Events {
    pub kafka_url: Option<Url>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Auth {
    pub keycloak_realm: Url,
    pub client_id: String,
    pub client_secret: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            #[cfg(debug_assertions)]
            rust_log: LogLevel::DEBUG,
            #[cfg(not(debug_assertions))]
            rust_log: LogLevel::INFO,
            storage: Storage {
                // It's fine to unwrap this because it shouldn't ever fail
                database_url: Url::from_str("postgresql://rematch:password@localhost/matchmaking")
                    .unwrap(),
                redis_url: Default::default(),
            },
            events: Events {
                kafka_url: Default::default(),
            },
            auth: Auth {
                // It's fine to unwrap this because it shouldn't ever fail
                keycloak_realm: Url::from_str("http://localhost:8080/auth/realms/rematch").unwrap(),
                client_id: "matchmaking".into(),
                client_secret: None,
            },
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
            .select(Profile::from_env_or("MM_PROFILE", "default"))
            .merge(Toml::file("Config.toml").nested())
            .merge(Env::prefixed("MM_").global());

        let mut config: Config = figment.extract()?;
        config.raw = Some(figment);

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // This test verifies that none of the default values cause the application to panic.
    // If this panics, then there's likely something wrong with the Urls in the config.
    #[test]
    fn test_config_default_doesnt_panic() {
        let _config = Config::default();

        assert!(true);
    }
}
