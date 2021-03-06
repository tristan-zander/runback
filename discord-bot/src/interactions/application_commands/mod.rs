pub mod admin;
pub mod eula;
pub mod lfg;
pub mod matchmaking;
pub mod utils;

pub use utils::ApplicationCommandUtilities;

use std::sync::Arc;

use entity::sea_orm::prelude::Uuid;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::MessageComponentInteraction,
    },
    channel::message::MessageFlags,
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder};
use twilight_util::builder::InteractionResponseDataBuilder;

use twilight_model::application::interaction::ApplicationCommand;

/// Describes a group of commands. This is mainly used
/// for structural purposes, and for the `/help` command
#[derive(Debug, Clone)]
pub struct CommandGroupDescriptor {
    /// The name of the command group
    pub name: &'static str,
    /// The description of the command group
    pub description: &'static str,
    /// The commands that are releated to this group
    pub commands: Box<[Command]>,
}

#[async_trait]
pub trait InteractionHandler {
    fn describe(&self) -> CommandGroupDescriptor;
    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()>;
    async fn process_autocomplete(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()>;
    async fn process_modal(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()>;
    async fn process_component(&self, data: Box<MessageComponentData>) -> anyhow::Result<()>;
}

pub struct ApplicationCommandData {
    pub command: ApplicationCommand,
    pub id: Uuid,
    // pub cancellation_token
}

pub struct MessageComponentData {
    pub message: MessageComponentInteraction,
    pub action: String,
    pub id: Uuid,
    // pub cancellation_token
}

#[derive(Debug)]
pub struct PingCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl InteractionHandler for PingCommandHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "ping".to_string(),
            "Responds with pong".into(),
            CommandType::ChatInput,
        )
        .option(StringBuilder::new(
            "text".into(),
            "Send this text alongside the response".into(),
        ));

        let command = builder.build();
        debug!(command = %format!("{:?}", command), "Created command");
        return CommandGroupDescriptor {
            name: "ping",
            description: "Commands that relate to response time",
            commands: Box::new([command]),
        };
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        let message = InteractionResponse {
            data: Some(
                InteractionResponseDataBuilder::new()
                    .embeds(vec![
                        EmbedBuilder::new()
                            .color(0x55_4e_2b)
                            .description("Runback Matchmaking Bot")
                            .field(EmbedFieldBuilder::new("Ping?", "Pong!").build())
                            .build(), // .map_err(|e| RunbackError {
                                      //     message: "Failed to build callback data".to_owned(),
                                      //     inner: Some(e.into()),
                                      // })?
                    ])
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            ),
            kind: InteractionResponseType::ChannelMessageWithSource,
        };

        let _res = self
            .utils
            .http_client
            .interaction(self.utils.application_id)
            .create_response(data.command.id, data.command.token.as_str(), &message)
            .exec()
            .await?;

        debug!(message = %format!("{:?}", message), "Reponded to command \"Pong\"");

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
