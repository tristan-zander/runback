use std::sync::Arc;

use crate::{
    db::RunbackDB,
    events::{Lobby, LobbyCommand},
    queries::lobby::LobbyQuery,
    services::LobbyService,
};

use cqrs_es::{persist::PersistedEventStore, CqrsFramework};
use postgres_es::PostgresEventRepository;
use sea_orm::prelude::Uuid;
use twilight_model::{
    application::interaction::application_command::CommandDataOption,
    guild::PartialMember,
    id::{
        marker::{ChannelMarker, UserMarker},
        Id,
    },
    user::User,
};

use crate::{client::DiscordClient, interactions::application_commands::ApplicationCommandData};

pub struct LobbyData {
    pub command: Box<ApplicationCommandData>,
    pub action: String,
    pub option: CommandDataOption,
    pub member: PartialMember,
    pub user: User,
}

impl AsRef<ApplicationCommandData> for LobbyData {
    fn as_ref(&self) -> &ApplicationCommandData {
        self.command.as_ref()
    }
}

pub struct LobbyCommandHandler {
    client: DiscordClient,
    db: RunbackDB,
    lobby_events: CqrsFramework<Lobby, PersistedEventStore<PostgresEventRepository, Lobby>>,
    lobby_store: PersistedEventStore<PostgresEventRepository, Lobby>,
}

impl LobbyCommandHandler {
    pub fn new(client: DiscordClient, db: RunbackDB) -> Self {
        let lobby_service = LobbyService::new(client.clone());

        let lobby_store = db.get_event_store();
        let query = Box::new(LobbyQuery::new(Arc::new(db.get_view_repository())));
        let lobby_events = CqrsFramework::new(db.get_event_store(), vec![query], lobby_service);

        Self {
            lobby_events,
            lobby_store,
            client,
            db,
        }
    }

    pub async fn process_command(&self, data: LobbyData) -> Result<(), anyhow::Error> {
        let interaction = data.command.interaction.clone();
        match data.action.as_str() {
            "open" => {
                self.open_lobby(data).await?;
            }
            "close" => {
                self.close_lobby(
                    data.command.user.id,
                    data.command
                        .interaction
                        .channel_id
                        .ok_or_else(|| anyhow!("Channel ID was expected but not found."))?,
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

        Ok(())
    }

    async fn open_lobby(&self, data: LobbyData) -> anyhow::Result<()> {
        let interaction = data.command.interaction;
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

    async fn has_opened_lobby(&self, user_id: Id<UserMarker>) -> Option<Lobby> {
        unimplemented!()
    }

    async fn invite(&self) {}

    async fn add_users_to_thread(
        &self,
        thread_id: Id<ChannelMarker>,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
    ) -> anyhow::Result<()> {
        self.client.join_thread(thread_id).await?;

        for user in users {
            self.client.add_thread_member(thread_id, *user).await?;
        }

        Ok(())
    }

    async fn close_lobby(
        &self,
        _owner: Id<UserMarker>,
        _channel: Id<ChannelMarker>,
    ) -> anyhow::Result<()> {
        unimplemented!()
    }
}
