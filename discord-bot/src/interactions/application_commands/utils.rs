use bot::entity::{matchmaking_settings, prelude::*};
use chrono::Utc;
use sea_orm::{prelude::*, DatabaseConnection, Set};
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::{
    application::interaction::Interaction,
    http::interaction::InteractionResponse,
    id::{
        marker::{ApplicationMarker, GuildMarker, UserMarker},
        Id,
    },
    user::User,
};

use twilight_http::{client::ClientBuilder, Client as DiscordHttpClient};
use twilight_standby::Standby;

use std::sync::Arc;

/// Contains any helper functions to help make writing application command handlers easier
// Make sure this is thread safe
#[derive(Debug)]
pub struct CommonUtilities {
    pub http_client: DiscordHttpClient,
    pub application_id: Id<ApplicationMarker>,
    pub db: Arc<Box<DatabaseConnection>>,
    pub cache: Arc<InMemoryCache>,
    pub standby: Arc<Standby>,
}

impl CommonUtilities {
    pub async fn new(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> anyhow::Result<Self> {
        let http_client = ClientBuilder::new()
            .token(crate::CONFIG.token.clone())
            .build();

        let application_id = {
            let response = http_client.current_user_application().exec().await?;
            response.model().await?.id
        };

        Ok(Self::new_with_application_id(
            db,
            application_id,
            cache,
            standby,
        ))
    }

    pub fn new_with_application_id(
        db: Arc<Box<DatabaseConnection>>,
        application_id: Id<ApplicationMarker>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> Self {
        Self {
            db,
            http_client: ClientBuilder::new()
                .token(crate::CONFIG.token.clone())
                .build(),
            application_id,
            cache,
            standby,
        }
    }

    pub fn db_ref(&self) -> &DatabaseConnection {
        (*self.db).as_ref()
    }

    pub async fn send_message(
        &self,
        interaction: &Interaction,
        message: &InteractionResponse,
    ) -> anyhow::Result<()> {
        let res = self
            .http_client
            .interaction(self.application_id)
            .create_response(interaction.id, interaction.token.as_str(), message)
            .exec()
            .await?;

        debug!("Send Message response: {:#?}", res);

        Ok(())
    }

    /// If the guild does not exist, it will create the settings with the default settings
    /// and commit it to the database.
    pub async fn get_guild_settings(
        &self,
        guild: Id<GuildMarker>,
    ) -> anyhow::Result<matchmaking_settings::Model> {
        use matchmaking_settings as settings;
        use MatchmakingSettings as Setting;

        let guild_id: IdWrapper<_> = guild.into();
        let setting = Setting::find_by_id(guild_id.clone())
            .one(self.db_ref())
            .await?;

        match setting {
            Some(setting) => Ok(setting),
            None => {
                let setting = settings::ActiveModel {
                    guild_id: Set(guild_id),
                    last_updated: Set(Utc::now()),
                    ..Default::default()
                };

                let setting = Setting::insert(setting)
                    .exec_with_returning(self.db_ref())
                    .await?;

                Ok(setting)
            }
        }
    }

    pub async fn get_user(&self, user: Id<UserMarker>) -> anyhow::Result<User> {
        if let Some(user_ref) = self.cache.user(user) {
            let user = user_ref.to_owned();
            return Ok(user);
        }

        let user = self.http_client.user(user).exec().await?.model().await?;
        Ok(user)
    }
}
