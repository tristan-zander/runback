use std::sync::Arc;

use bot::{
    entity::prelude::*,
    events::{Lobby, LobbyCommand},
    services::LobbyService,
};

use chrono::Utc;
use cqrs_es::{persist::PersistedEventStore, CqrsFramework};
use postgres_es::PostgresEventRepository;
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
}

impl LobbyCommandHandler {
    pub async fn new(utils: Arc<CommonUtilities>) -> Self {
        let lobby_service = LobbyService::new(
            ClientBuilder::new()
                .token(crate::CONFIG.token.to_owned())
                .build(),
        );
        let lobby_events = create_event_handler::<Lobby>(lobby_service).await;

        Self {
            utils,
            lobby_events,
        }
    }

    pub async fn process_command(&self, data: LobbyData) -> Result<(), anyhow::Error> {
        let interaction = data.data.interaction.clone();
        match data.action.as_str() {
            "open" => {
                self.open_lobby(data).await?;
            }
            "close" => {
                let lobby = self.get_lobby(data.data.guild_id).await?;
                self.close_lobby(&lobby).await?;
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

    async fn get_lobby(
        &self,
        _guild_id: Id<GuildMarker>,
    ) -> Result<matchmaking_lobbies::Model, anyhow::Error> {
        unimplemented!()
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

    async fn close_lobby(&self, lobby: &matchmaking_lobbies::Model) -> anyhow::Result<()> {
        let _update_res = matchmaking_invitation::Entity::update_many()
            .filter(matchmaking_invitation::Column::Lobby.eq(lobby.id))
            .filter(matchmaking_invitation::Column::ExpiresAt.gt(Utc::now()))
            .set(matchmaking_invitation::ActiveModel {
                expires_at: Set(Utc::now()),
                ..Default::default()
            })
            .exec(self.utils.db_ref())
            .await?;

        let _update_res = MatchmakingLobbies::update(matchmaking_lobbies::ActiveModel {
            id: Set(lobby.id),
            ended_at: Set(Some(Utc::now())),
            ..Default::default()
        })
        .exec(self.utils.db_ref())
        .await?;
        Ok(())
    }
}
