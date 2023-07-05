pub mod application_commands;
pub mod panels;

use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::entity::sea_orm::prelude::Uuid;

use futures::future::BoxFuture;
use tokio::time::timeout;
use tracing::{Instrument, Level};
use twilight_gateway::Shard;
use twilight_model::{
    application::{command::Command, interaction::InteractionData},
    channel::message::MessageFlags,
    gateway::payload::incoming::InteractionCreate,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{CommandMarker, GuildMarker},
        Id,
    },
};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder};

use crate::{
    client::DiscordClient,
    interactions::application_commands::{ApplicationCommandData, MessageComponentData},
};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, matchmaking::MatchmakingCommandHandler,
    CommandGroupDescriptor, InteractionHandler,
};

type HandlerType = Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>;

pub struct InteractionProcessor {
    application_command_handlers: HashMap<Id<CommandMarker>, HandlerType>,
    component_handlers: HashMap<&'static str, HandlerType>,
    commands: Vec<Command>,
    command_groups: Vec<CommandGroupDescriptor>,
    discord_client: DiscordClient,
}

impl InteractionProcessor {
    pub fn new(discord_client: DiscordClient) -> Self {
        Self {
            application_command_handlers: Default::default(),
            component_handlers: Default::default(),
            commands: Default::default(),
            command_groups: Default::default(),
            discord_client,
        }
    }

    #[instrument(name = "init interaction processor", skip_all)]
    pub async fn init(&mut self, debug_guild: Option<Id<GuildMarker>>) -> anyhow::Result<()> {
        event!(Level::INFO, "Registering top-level command handlers");

        let top_level_handlers = [
            AdminCommandHandler::describe(),
            MatchmakingCommandHandler::describe(),
        ];

        self.register_commands(top_level_handlers, debug_guild)
            .await?;

        Ok(())
    }

    // Man this is so ugly
    async fn register_commands<T>(
        &mut self,
        describes: T,
        debug_guild_id: Option<Id<GuildMarker>>,
    ) -> anyhow::Result<()>
    where
        T: IntoIterator<Item = CommandGroupDescriptor>,
    {
        self.commands = {
            let commands = describes
                .into_iter()
                .flat_map(|d| d.commands.as_ref().to_owned())
                .collect::<Vec<_>>();

            if let Some(debug_guild) = debug_guild_id {
                self.discord_client
                    .interaction()
                    .set_guild_commands(debug_guild, commands.as_slice())
                    .await?
                    .models()
                    .await?
            } else {
                self.discord_client
                    .interaction()
                    .set_global_commands(commands.as_slice())
                    .await?
                    .models()
                    .await?
            }
        };

        debug!(commands = ?serde_json::to_string(&self.commands).unwrap(), "Sent commands to discord");

        // for descriptor in describes {
        //     debug!(desc = ?descriptor, name = ?descriptor.name, "descriptor");
        //     let old = self
        //         .component_handlers
        //         .insert(descriptor.name, handler.clone());

        //     info!(name = ?descriptor.name, "inserting command handler");

        //     if let Some(_) = old {
        //         return Err(anyhow!(
        //             "tried to overwrite a component handler: {}",
        //             descriptor.name
        //         ));
        //     }

        //     if let Some(command) = self.commands.iter().find(|c| c.name == descriptor.name) {
        //         if let Some(old) = self.application_command_handlers.insert(
        //             command
        //                 .id
        //                 .ok_or_else(|| anyhow!("command does not have an id: {}", command.name))?,
        //             handler.clone(),
        //         ) {
        //             return Err(anyhow!(
        //                 "inserted a handler over a command... {:#?}",
        //                 old.describe().name
        //             ));
        //         }
        //     } else {
        //         warn!(name = ?descriptor.name, "no command found");
        //     }
        // }

        // Commands that are currently in Discord
        let global_commands = self
            .discord_client
            .interaction()
            .global_commands()
            .await?
            .models()
            .await?;

        for c in global_commands {
            if self.commands.iter().any(|new_c| c.id == new_c.id) {
                debug!(name = ?c.name, id = ?c.id, "found matching command");
            } else {
                // Remove the command from Discord. We're no longer going to support it
                warn!(name = ?c.name, id = ?c.id, "unknown global command found on Discord");
                self.discord_client
                    .interaction()
                    .delete_global_command(c.id.ok_or_else(|| anyhow!("global command has no ID"))?) // realistically, this error will never be shown
                    .await?;
            }
        }

        Ok(())
    }

    pub fn handle_interaction<'shard>(
        &self,
        interaction: Box<InteractionCreate>,
        _shard: &'shard Shard,
    ) -> anyhow::Result<BoxFuture<'static, anyhow::Result<()>>> {
        event!(tracing::Level::DEBUG, "Received interaction");

        // TODO: Send a deferred message response, followup on it later

        let interaction_data = if let Some(data) = &interaction.data {
            data
        } else {
            return Ok(Box::pin(async { return Ok(()) }));
        };

        match interaction_data {
            // I think this is only for webhook interaction handlers
            // twilight_model::application::interaction::Interaction::Ping(_) => ,
            InteractionData::ApplicationCommand(command) => {
                // self.application_command_handlers
                //     .on_command_receive(command.as_ref())
                //     .await?;
                debug!(id = ?&command.id, "received application command");
                if let Some(handler) = self.application_command_handlers.get(&command.id) {
                    let data = Box::new(ApplicationCommandData::new(
                        *command.clone(),
                        interaction.0.clone(),
                    )?);
                    let fut = Box::pin(Self::execute_application_command(
                        handler.clone(),
                        data,
                        self.discord_client.clone(),
                    ));
                    return Ok(fut);
                } else {
                    error!(name = ?command.name,"No command handler found");
                    return Err(anyhow!("No such command handler found"));
                }
            }
            InteractionData::MessageComponent(message) => {
                debug!("Received message component");

                if let Some((handler_name, leftover)) = message.custom_id.split_once(':') {
                    if let Some(handler) = self.component_handlers.get(handler_name) {
                        let data = Box::new(MessageComponentData {
                            id: Uuid::new_v4(),
                            message: message.clone(),
                            action: leftover.to_string(),
                            interaction: interaction.0.clone(),
                        });
                        let handler = handler.clone();
                        let fut = Box::pin(async move { handler.process_component(data).await });
                        return Ok(fut);
                    } else {
                        return Err(anyhow!(
                            "Invalid message component handler: {}",
                            handler_name
                        ));
                    }
                } else {
                    return Err(anyhow!(
                        "Message component custom_id does not match the format \"handler:action\""
                    ));
                }
            }
            InteractionData::ModalSubmit(_modal) => {
                debug!("Received modal");
            }
            _ => {
                debug!(interaction = %format!("{:?}", interaction), "Unhandled interaction");
            }
        }

        trace!("Ended interaction response.");
        Err(anyhow!("Interaction was unmatched"))
    }

    async fn execute_application_command(
        handler: HandlerType,
        data: Box<ApplicationCommandData>,
        discord_client: DiscordClient,
    ) -> anyhow::Result<()> {
        discord_client
            .interaction()
            .create_response(
                data.interaction.id,
                data.interaction.token.as_str(),
                &InteractionResponse {
                    kind: InteractionResponseType::DeferredChannelMessageWithSource,
                    data: None,
                },
            )
            .await?;

        let name = data.command.name.clone();
        let token = data.interaction.token.clone();
        let runback_id = data.id;
        let timeout = timeout(
            Duration::from_secs(5),
            handler
                .process_command(data)
                .instrument(info_span!("command_exec")),
        );
        let res = timeout.await;
        match res {
            Ok(res) => {
                if let Err(e) = res {
                    error!(error = ?e, "Application Command Failed");
                    debug!(error = ?format!("{:?}", e), "Application Command Failed");
                    discord_client
                        .interaction()
                        .create_followup(token.as_str())
                        .flags(MessageFlags::EPHEMERAL)
                        .embeds(&[EmbedBuilder::new()
                            .description("An error has occurred.")
                            .footer(
                                EmbedFooterBuilder::new(runback_id.hyphenated().to_string())
                                    .build(),
                            )
                            .field(EmbedFieldBuilder::new("error", e.to_string()).build())
                            .validate()?
                            .build()])?
                        .await?;
                }

                Ok(())
            }
            Err(_) => {
                discord_client
                    .interaction()
                    .update_response(token.as_str())
                    .content(Some("Command timed out."))?
                    .await?;
                Err(anyhow!("Command timed out: {}", name))
            }
        }
        // handler.process_command(data).await
    }
}
