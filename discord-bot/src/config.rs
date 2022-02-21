use common::logging::LogLevel;
use figment::{
    providers::{Env, Format, Serialized, Toml},
    Figment, Profile,
};
use serde::{Deserialize, Serialize};
use std::error::Error;
use twilight_model::id::{marker::GuildMarker, Id};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub token: String,
    pub debug_guild_id: Option<Id<GuildMarker>>,
    pub log_level: LogLevel,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            token: Default::default(),
            debug_guild_id: Default::default(),
            log_level: Default::default(),
        }
    }
}

impl Config {
    pub fn new() -> Result<Config, Box<dyn Error>> {
        Ok(Self::figment().extract()?)
    }

    pub fn figment() -> Figment {
        Figment::from(Serialized::defaults(Config::default()))
            .select(Profile::from_env_or("BOT_PROFILE", "default"))
            .merge(Toml::file("Bot.toml").nested())
            .merge(Env::prefixed("BOT_").global())
    }
}
