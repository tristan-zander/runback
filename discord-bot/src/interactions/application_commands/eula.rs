use std::sync::Arc;

use crate::entity::prelude::*;

use chrono::Utc;
use sea_orm::{prelude::*, IntoActiveModel, Set};
use twilight_model::{
    application::{command::CommandType, interaction::application_command::CommandOptionValue},
    channel::message::MessageFlags,
    guild::Permissions,
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder};
use twilight_util::builder::InteractionResponseDataBuilder as CallbackDataBuilder;

use crate::{
    client::{DiscordClient, RunbackClient},
    db::RunbackDB,
};

use super::{
    ApplicationCommandData, CommandGroupDescriptor, InteractionHandler, MessageComponentData,
};

// TODO: Make a distinct EULA for the bot itself
const EULA: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../", "EULA.md"));

pub struct EulaCommandHandler {
    db: RunbackDB,
    client: DiscordClient,
}

#[async_trait]
impl InteractionHandler for EulaCommandHandler {
    fn create(client: &RunbackClient) -> Self {
        Self {
            db: client.db(),
            client: client.discord(),
        }
    }

    fn describe() -> CommandGroupDescriptor {
        let builder = CommandBuilder::new("eula", "Show the EULA", CommandType::ChatInput)
            .dm_permission(false)
            .default_member_permissions(Permissions::MANAGE_GUILD)
            .option(
                StringBuilder::new("accept", "Accept the EULA (admin only)").choices(vec![(
                    "I have read the EULA and agree to its terms.",
                    "accept",
                )]),
            );

        let command = builder.build();
        CommandGroupDescriptor {
            name: "eula",
            description: "Read and accept the EULA",
            commands: Box::new([command]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        let command = &data.command;
        debug!(options = %format!("{:?}", command.options));

        let gid = if let Some(gid) = command.guild_id {
            gid
        } else {
            let message = InteractionResponse {
                data: Some(
                    CallbackDataBuilder::new()
                        .content("You cannot use this command in a DM.")
                        .flags(MessageFlags::EPHEMERAL)
                        .build(),
                ),
                kind: InteractionResponseType::ChannelMessageWithSource,
            };
            self.client
                .send_message(&data.interaction, &message)
                .await
                .map_err(|e| anyhow!("Could not send message: {}", e))?;

            return Ok(());
        };

        let options = &command.options;
        if !options.is_empty() && options[0].name.as_str() == "accept" {
            match &options[0].value {
                CommandOptionValue::String(accepted) => {
                    if accepted.as_str() != "accept" {
                        let message = InteractionResponse {
                            data: Some(
                            CallbackDataBuilder::new()
                                .content("You must accept the EULA to use Runback. Run \"/eula\" without any arguments to see the EULA.")
                                .flags(MessageFlags::EPHEMERAL)
                                .build()
                        ),
                            kind: InteractionResponseType::ChannelMessageWithSource,
                    };
                        self.client
                            .send_message(&data.interaction, &message)
                            .await
                            .map_err(|e| anyhow!("Could not send message: {}", e))?;

                        error!(
                            accepted = %accepted.clone(),
                            "Received unexpected value instead of accepting the EULA"
                        );

                        return Ok(());
                    }

                    let res = MatchmakingSettings::find_by_id(gid.into())
                        .one(self.db.connection())
                        .await?;
                    match res {
                        Some(existing_settings) => {
                            if existing_settings.has_accepted_eula.is_some() {
                                let message = InteractionResponse {
                                    data: Some(
                                        CallbackDataBuilder::new()
                                            .content("Looks like you've already accepted the EULA.")
                                            .flags(MessageFlags::EPHEMERAL)
                                            .build(),
                                    ),
                                    kind: InteractionResponseType::ChannelMessageWithSource,
                                };
                                self.client
                                    .send_message(&data.interaction, &message)
                                    .await
                                    .map_err(|e| anyhow!("Could not send message: {}", e))?;
                                return Ok(());
                            } else {
                                let mut active = existing_settings.into_active_model();
                                active.has_accepted_eula = Set(Some(Utc::now()));
                                let db_ref = self.db.connection();
                                async move { active.update(db_ref).await }.await?;
                            }
                        }
                        None => {
                            let settings = matchmaking_settings::ActiveModel {
                                guild_id: Set(gid.into()),
                                has_accepted_eula: Set(Some(Utc::now())),
                                last_updated: Set(Utc::now()),
                                ..Default::default()
                            };

                            settings.insert(self.db.connection()).await?;
                        }
                    };

                    let message = InteractionResponse { kind: InteractionResponseType::ChannelMessageWithSource, data: Some(
                            CallbackDataBuilder::new()
                                .content("Okay, thanks for accepted the EULA. You may now use Runback's services.")
                                .flags(MessageFlags::EPHEMERAL)
                                .build(),
                        )};
                    self.client
                        .send_message(&data.interaction, &message)
                        .await
                        .map_err(|e| anyhow!("Could not send message: {}", e))?;
                }
                _ => {}
            }
        }

        let message = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
                CallbackDataBuilder::new()
                    .content(EULA)
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            ),
        };

        self.client
            .send_message(&data.interaction, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?;

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
