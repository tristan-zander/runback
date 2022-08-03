use chrono::Utc;
use dashmap::DashMap;
use entity::sea_orm::prelude::{DateTimeUtc, Uuid};
use twilight_model::{
    application::command::{BaseCommandOptionData, CommandOption, CommandType},
    channel::{
        message::{allowed_mentions::AllowedMentionsBuilder, AllowedMentions},
        thread::AutoArchiveDuration,
        Channel, ChannelType,
    },
    http::interaction::{InteractionResponse, InteractionResponseData, InteractionResponseType},
    id::{
        marker::{ChannelMarker, GuildMarker, InteractionMarker, UserMarker},
        Id,
    },
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder},
    embed::{EmbedBuilder, EmbedFieldBuilder},
    InteractionResponseDataBuilder,
};

use super::{
    ApplicationCommandData, ApplicationCommandUtilities, CommandGroupDescriptor,
    InteractionHandler, MessageComponentData,
};

use std::sync::Arc;

#[derive(Debug, Clone)]
struct Session {
    id: Uuid,
    users: Vec<Id<UserMarker>>,
    thread: Id<ChannelMarker>,
    started_at: DateTimeUtc,
}

#[derive(Debug, Clone)]
pub struct MatchmakingCommandHandler {
    utils: Arc<ApplicationCommandUtilities>,
    sessions: DashMap<Id<ChannelMarker>, Session>,
}

#[async_trait]
impl InteractionHandler for MatchmakingCommandHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "matchmaking".to_string(),
            "Matchmaking commands".to_string(),
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new(
                "play-against".to_string(),
                "Start a match with an opponent".to_string(),
            )
            .option(CommandOption::User(BaseCommandOptionData {
                name: "opponent".to_string(),
                description: "The user that you wish to play against".to_string(),
                description_localizations: None,
                name_localizations: None,
                required: true,
            }))
            .build(),
        )
        // .option(
        //     SubCommandBuilder::new("show-matches".into(), "Show the matchmaking menu".into())
        //         .build(),
        // )
        .option(
            SubCommandBuilder::new(
                "settings".into(),
                "View and update settings such as default character".into(),
            )
            .build(),
        )
        .option(
            SubCommandBuilder::new("done".into(), "Finish your matchmaking session".into()).build(),
        );

        let command = builder.build();
        CommandGroupDescriptor {
            name: "matchmaking",
            description: "Commands that are related to matchmaking",
            commands: Box::new([command]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        if let Some(_target) = data.command.data.target_id {
            // Then start a mm session with that user. It's not chat message command,
            // but a click interaction.
        }

        let mut users = data
            .command
            .data
            .resolved
            .as_ref()
            .into_iter()
            .flat_map(|r| r.users.keys().map(|id| id.to_owned()).collect::<Vec<_>>())
            .collect::<Vec<_>>();

        let owner_id = data
            .command
            .member
            .as_ref()
            .ok_or_else(|| anyhow!("Command cannot be used in a DM"))?
            .user
            .as_ref()
            .ok_or_else(|| {
                anyhow!("Could not get the Discord member's user field (structure is partial)")
            })?
            .id;

        users.push(owner_id);

        if users.len() == 0 {
            return Err(anyhow!(
                "Cannot start matchmaking without specifying an opponent"
            ));
        }

        let thread = self
            .start_matchmaking_thread(
                data.command
                    .guild_id
                    .ok_or_else(|| anyhow!("Command cannot be run in a DM"))?,
                "Matchmaking test",
            )
            .await?;

        let res = self.add_users_to_thread(thread.id, &users).await;

        if let Err(e) = res {
            // Close the thread and send an error.

            self.utils
                .http_client
                .delete_channel(thread.id)
                .exec()
                .await?;

            return Err(e);
        }

        self.send_thread_opening_message(&users, thread.id).await?;

        let session = Session {
            id: Uuid::new_v4(),
            users,
            thread: thread.id,
            started_at: Utc::now(),
        };

        self.sessions.insert(thread.id, session);

        Ok(())
    }

    async fn process_autocomplete(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_modal(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_component(&self, _data: Box<MessageComponentData>) -> anyhow::Result<()> {
        unreachable!()
    }
}

impl MatchmakingCommandHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        // TODO: Start a thread to keep track of the matchmaking instances.

        Self {
            utils,
            sessions: DashMap::new(),
        }
    }

    async fn send_thread_opening_message(
        &self,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
        channel: Id<ChannelMarker>,
    ) -> anyhow::Result<()> {
        let msg = self
            .utils
            .http_client
            .create_message(channel)
            .allowed_mentions(Some(
                &AllowedMentionsBuilder::new()
                    .user_ids(users.into_iter().map(|id| id.to_owned()))
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
            .exec()
            .await?;

        Ok(())
    }

    async fn start_matchmaking_thread(
        &self,
        guild: Id<GuildMarker>,
        name: &str,
    ) -> anyhow::Result<Channel> {
        let settings = self.utils.get_guild_settings(guild).await?;

        if let Some(channel) = settings.channel_id {
            let channel = channel.into_id();

            let thread = self
                .utils
                .http_client
                .create_thread(channel, name, ChannelType::GuildPublicThread)?
                .invitable(true)
                // archive in 3 hours
                .auto_archive_duration(AutoArchiveDuration::Day)
                .exec()
                .await?
                .model()
                .await?;

            return Ok(thread);
        }

        Err(anyhow!(
            "The server has not enabled a default matchmaking channel"
        ))
    }

    async fn add_users_to_thread(
        &self,
        thread_id: Id<ChannelMarker>,
        users: impl IntoIterator<Item = &Id<UserMarker>>,
    ) -> anyhow::Result<()> {
        self.utils.http_client.join_thread(thread_id).exec().await?;

        for user in users {
            self.utils
                .http_client
                .add_thread_member(thread_id, *user)
                .exec()
                .await?;
        }

        Ok(())
    }
}
