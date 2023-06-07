use std::sync::Arc;

use bot::{entity::prelude::*, services::LobbyService, events::{Lobby, LobbyCommand}};

use chrono::Utc;
use cqrs_es::{CqrsFramework, persist::PersistedEventStore};
use postgres_es::PostgresEventRepository;
use sea_orm::prelude::Uuid;
use twilight_http::client::ClientBuilder;
use twilight_model::{
    application::interaction::application_command::CommandDataOption,
    channel::{
        message::allowed_mentions::AllowedMentionsBuilder, thread::AutoArchiveDuration, Channel,
        ChannelType::PublicThread,
    },
    guild::PartialMember,
    id::{
        marker::{ChannelMarker, GuildMarker, UserMarker},
        Id,
    },
};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder};

use crate::{interactions::application_commands::{ApplicationCommandData, CommonUtilities}, create_event_handler};

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
    lobby_events: CqrsFramework<Lobby, PersistedEventStore<PostgresEventRepository, Lobby>>
}

impl LobbyCommandHandler {
    pub async fn new(utils: Arc<CommonUtilities>) -> Self {
        let lobby_service = LobbyService::new(
            ClientBuilder::new().token(crate::CONFIG.token.to_owned()).build()
        );
        let lobby_events = create_event_handler::<Lobby>(lobby_service).await;

        Self { utils, lobby_events }
    }

    pub async fn process_command(&self, data: LobbyData) -> Result<(), anyhow::Error> {
        match data.action.as_str() {
            "open" => {
                let interaction = data.data.interaction;
                let channel = interaction.channel_id.unwrap();
                let thread = self
                    .start_matchmaking_thread(data.data.guild_id, channel)
                    .await?;

                self.send_thread_opening_message(
                    &[data
                        .member
                        .user
                        .ok_or_else(|| anyhow!("could not get user id"))?
                        .id],
                    thread.id,
                )
                .await?;

                self.lobby_events.execute("2", LobbyCommand::OpenLobby { owner_id: 0, channel: 0 }).await.map_err(|e| anyhow!(e))?;

                return Err(anyhow!("I just haven't gotten this far yet."));

                unimplemented!("Give the user feedback that the session has started.")
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

        Ok(())
    }

    async fn get_lobby(
        &self,
        guild_id: Id<GuildMarker>,
    ) -> Result<matchmaking_lobbies::Model, anyhow::Error> {
        unimplemented!()
    }

    async fn invite(&self) {}

    async fn send_thread_opening_message(
        &self,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
        channel: Id<ChannelMarker>,
    ) -> anyhow::Result<()> {
        let _msg = self
            .utils
            .http_client
            .create_message(channel)
            .allowed_mentions(Some(
                &AllowedMentionsBuilder::new()
                    .user_ids(users.into_iter().copied())
                    .build(),
            ))
            .embeds(&[EmbedBuilder::new()
                .description(
                    "**Thank you for using Runback. \
                        Below are a list of commands to assist you during your matches.**",
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking report",
                        "Report the score for your match",
                    )
                    .build(),
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking done",
                        "Finish matchmaking and finalize results",
                    )
                    .build(),
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking settings",
                        "Set the settings of the lobby.",
                    )
                    .build(),
                )
                .validate()?
                .build()])?
            .await?;

        Ok(())
    }

    async fn start_matchmaking_thread(
        &self,
        guild: Id<GuildMarker>,
        channel: Id<ChannelMarker>,
    ) -> anyhow::Result<Channel> {
        let settings = self.utils.get_guild_settings(guild).await?;

        let channel = if let Some(channel) = settings.channel_id {
            channel.into_id()
        } else {
            channel
        };

        let thread = self
            .utils
            .http_client
            .create_thread(channel, "dummy channel", PublicThread)?
            .auto_archive_duration(AutoArchiveDuration::Day)
            .await?
            .model()
            .await?;

        return Ok(thread);
    }

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
