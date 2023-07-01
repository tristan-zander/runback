use bot::entity::prelude::*;
use chrono::Utc;
use sea_orm::{prelude::*, DatabaseConnection, IntoActiveModel, Set};
use twilight_cache_inmemory::InMemoryCache;
use twilight_model::{
    application::interaction::Interaction,
    guild::Guild,
    http::interaction::{InteractionResponse, InteractionResponseData},
    id::{
        marker::{ApplicationMarker, GuildMarker, InteractionMarker, UserMarker},
        Id,
    },
    user::{CurrentUser, User},
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
    pub current_user: CurrentUser,
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

        let current_user = http_client.current_user().exec().await?.model().await?;

        Ok(Self::new_with_fields(
            db,
            application_id,
            current_user,
            cache,
            standby,
        ))
    }

    pub fn new_with_fields(
        db: Arc<Box<DatabaseConnection>>,
        application_id: Id<ApplicationMarker>,
        current_user: CurrentUser,
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
            current_user,
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

    pub async fn find_or_create_user(&self, id: Id<UserMarker>) -> anyhow::Result<users::Model> {
        let res = Users::find()
            .filter(users::Column::DiscordUser.eq(IdWrapper::from(id)))
            .one(self.db_ref())
            .await?;

        if let Some(user) = res {
            Ok(user)
        } else {
            let user = users::Model {
                user_id: Uuid::new_v4(),
                discord_user: Some(id.into()),
            };

            let user = Users::insert(user.into_active_model())
                .exec_with_returning(self.db_ref())
                .await?;

            return Ok(user);
        }
    }

    pub async fn get_guild(&self, guild_id: Id<GuildMarker>) -> anyhow::Result<Guild> {
        let res = self.http_client.guild(guild_id).await?;
        return Ok(res.model().await.map_err(|e| anyhow!(e))?);
    }

    pub async fn respond_to_user(&self) -> anyhow::Result<()> {
        unimplemented!()
    }

    pub async fn ack(&self, interaction_token: &str) -> anyhow::Result<()> {
        self
            .http_client
            .interaction(self.application_id)
            .delete_response(interaction_token)
            .await?;

        Ok(())
    }
}
