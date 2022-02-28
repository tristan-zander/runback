use common::logging::LogLevel;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment, Profile,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use tracing_subscriber::registry::Data;
use twilight_model::id::{marker::GuildMarker, Id};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub token: String,
    pub debug_guild_id: Option<Id<GuildMarker>>,
    pub log_level: LogLevel,
    pub db: DatabaseSettings,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseSettings {
    pub protocol: String,
    pub username: String,
    pub password: Option<String>,
    pub host: String,
    pub db_name: String,
    // Extra arguments that are added to the connection string.
    pub extra_options: String
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            protocol: "postgres".to_owned(),
            username: "runback".to_owned(),
            password: None,
            // Only useful when using Docker Compose
            host: "db".to_owned(),
            db_name: "discord-client".to_owned(),
            extra_options: "".to_owned(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: Default::default(),
            debug_guild_id: Default::default(),
            log_level: Default::default(),
            db: Default::default(),
        }
    }
}

impl Config {
    pub fn new() -> Result<Config, figment::Error> {
        Ok(Self::figment().extract()?)
    }

    pub fn figment() -> Figment {
        Figment::from(Serialized::defaults(Config::default()))
            .select(Profile::from_env_or("BOT_PROFILE", "default"))
            .merge(Toml::file("Bot.toml").nested())
            .merge(Env::prefixed("BOT_").global())
    }
}
