use twilight_model::{
    application::command::{BaseCommandOptionData, CommandOption, CommandType},
    channel::{thread::AutoArchiveDuration, Channel, ChannelType},
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use super::{
    ApplicationCommandData, ApplicationCommandUtilities, CommandGroupDescriptor,
    InteractionHandler, MessageComponentData,
};

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MatchmakingCommandHandler {
    utils: Arc<ApplicationCommandUtilities>,
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
        );
        // .option(
        //     SubCommandBuilder::new("show-matches".into(), "Show the matchmaking menu".into())
        //         .build(),
        // )
        // .option(
        //     SubCommandBuilder::new(
        //         "settings".into(),
        //         "View and update settings such as default character".into(),
        //     )
        //     .build(),
        // )
        // .option(
        //     SubCommandBuilder::new(
        //         "end-session".into(),
        //         "Finish your matchmaking session".into(),
        //     )
        //     .build(),
        // );

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

        let users = data
            .command
            .data
            .resolved
            .into_iter()
            .flat_map(|r| r.users.into_keys().collect::<Vec<_>>())
            .collect::<Vec<_>>();

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

        self.utils.http_client.join_thread(thread.id).exec().await?;
        self.utils
            .http_client
            .add_thread_member(
                thread.id,
                data.command
                    .member
                    .ok_or_else(|| anyhow!("Command cannot be used in a DM"))?
                    .user
                    .ok_or_else(|| {
                        anyhow!(
                            "Could not get the Discord member's user field (structure is partial)"
                        )
                    })?
                    .id,
            )
            .exec()
            .await?;

        for user in users {
            self.utils
                .http_client
                .add_thread_member(thread.id, user)
                .exec()
                .await?;
        }

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
        Self { utils }
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
}
