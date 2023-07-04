use std::sync::Arc;

use bot::{
    entity::prelude::*,
    events::{Lobby, LobbyCommand},
    queries::LobbyQuery,
    services::LobbyService,
};

use chrono::Utc;
use cqrs_es::{
    persist::{GenericQuery, PersistedEventStore},
    CqrsFramework,
};
use postgres_es::{PostgresEventRepository, PostgresViewRepository};
use sea_orm::prelude::Uuid;
use twilight_http::client::ClientBuilder;
use twilight_model::{
    application::interaction::application_command::CommandDataOption,
    guild::PartialMember,
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
};

use crate::{
    create_event_handler,
    interactions::application_commands::{ApplicationCommandData, CommonUtilities},
};

pub struct LobbyData {
    pub data: Box<ApplicationCommandData>,
    pub action: String,
    pub option: CommandDataOption,
    pub member: PartialMember,
}

impl AsRef<ApplicationCommandData> for LobbyData {
    fn as_ref(&self) -> &ApplicationCommandData {
        self.data.as_ref()
    }
}

pub struct LobbyCommandHandler {
    utils: Arc<CommonUtilities>,
    lobby_events: CqrsFramework<Lobby, PersistedEventStore<PostgresEventRepository, Lobby>>,
    lobby_view: LobbyQuery,
}

impl LobbyCommandHandler {
    pub async fn new(utils: Arc<CommonUtilities>) -> Self {
        let lobby_service = LobbyService::new(
            ClientBuilder::new()
                .token(crate::CONFIG.token.to_owned())
                .build(),
        );
        let lobby_events = create_event_handler::<Lobby>(lobby_service).await;
        let lobby_view = todo!();

        Self {
            utils,
            lobby_events,
            lobby_view,
        }
    }

    pub async fn process_command(&self, data: LobbyData) -> Result<(), anyhow::Error> {
        let interaction = data.data.interaction.clone();
        match data.action.as_str() {
            "open" => {
                self.open_lobby(data).await?;
            }
            "close" => {
                self.close_lobby(
                    data.data.user.id,
                    data.data
                        .interaction
                        .channel_id
                        .ok_or_else(|e| anyhow!("Channel ID was expected but not found."))?,
                )
                .await?;
            }
            "settings" => {
                unimplemented!()
            }
            "invite" => {
                unimplemented!()
            }
            _ => return Err(anyhow!("")),
        }

        self.utils.ack(&interaction.token).await?;

        Ok(())
    }

    async fn open_lobby(&self, data: LobbyData) -> anyhow::Result<()> {
        let interaction = data.data.interaction;
        let channel = interaction.channel_id.unwrap();

        let owner_id = data
            .member
            .user
            .ok_or_else(|| anyhow!("could not get user id"))?
            .id;

        // Send a command to open a lobby.
        self.lobby_events
            .execute(
                Uuid::new_v4().to_string().as_str(),
                LobbyCommand::OpenLobby {
                    owner_id: owner_id.get(),
                    channel: channel.get(),
                },
            )
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(())
    }

    async fn invite(&self) {}

    async fn add_users_to_thread(
        &self,
        thread_id: Id<ChannelMarker>,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
    ) -> anyhow::Result<()> {
        self.utils.http_client.join_thread(thread_id).await?;

        for user in users {
            self.utils
                .http_client
                .add_thread_member(thread_id, *user)
                .await?;
        }

        Ok(())
    }

    async fn close_lobby(
        &self,
        owner: Id<UserMarker>,
        channel: Id<ChannelMarker>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }
}
