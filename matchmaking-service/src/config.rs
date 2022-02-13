use std::{collections::HashMap, error::Error, path::Path, str::FromStr};

use common::logging::{LogDriver, LogLevel};
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment, Profile,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub logging: Logging,
    pub storage: Storage,
    pub events: Events,
    pub auth: Auth,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Storage {
    pub database_url: Url,
    pub redis_url: Option<Url>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Events {
    pub kafka_settings: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Auth {
    pub keycloak_realm: Url,
    pub client_id: String,
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Logging {
    log_level: LogLevel,
    log_driver: LogDriver,
    log_to_file: Option<Box<Path>>,
    /// Specifically, add extra information about stats like thread ID, file name, etc. Only useful for debugging
    too_much_information: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            logging: Logging {
                #[cfg(debug_assertions)]
                log_level: LogLevel::DEBUG,
                #[cfg(not(debug_assertions))]
                log_level: LogLevel::INFO,
                log_driver: LogDriver::Print,
                log_to_file: None,
                #[cfg(not(debug_assertions))]
                too_much_information: false,
                #[cfg(debug_assertions)]
                too_much_information: true,
            },
            storage: Storage {
                // It's fine to unwrap this because it shouldn't ever fail
                database_url: Url::from_str("postgresql://rematch:password@localhost/matchmaking")
                    .unwrap(),
                redis_url: Default::default(),
            },
            events: Events {
                kafka_settings: HashMap::from([
                    ("bootstrap.servers".into(), "kafka:9092".into()),
                    ("message.timeout.ms".into(), "5000".into()),
                ]),
            },
            auth: Auth {
                // It's fine to unwrap this because it shouldn't ever fail
                keycloak_realm: Url::from_str("http://localhost:8080/auth/realms/rematch").unwrap(),
                client_id: "matchmaking".into(),
                client_secret: None,
            },
        }
    }
}

impl Config {
    pub fn new() -> Result<Config, Box<dyn Error>> {
        Ok(Self::figment().extract::<Config>()?)
    }

    /// Load order:
    ///     1. Set default values
    ///     2. Config.toml (override)
    ///     3. Any environment variables (override)
    pub fn figment() -> Figment {
        Figment::from(Serialized::defaults(Config::default()))
            .select(Profile::from_env_or("MM_PROFILE", "default"))
            .merge(Toml::file("Config.toml").nested())
            .merge(Env::prefixed("MM_").global())
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
