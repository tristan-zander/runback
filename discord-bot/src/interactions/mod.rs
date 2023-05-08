pub mod application_commands;
pub mod panels;

use std::{collections::HashMap, sync::Arc, time::Duration};

use bot::entity::sea_orm::prelude::Uuid;

use futures::future::BoxFuture;
use tokio::time::timeout;
use tracing::{Instrument, Level};
use twilight_gateway::Shard;
use twilight_model::{
    application::{command::Command, interaction::InteractionData},
    channel::message::MessageFlags,
    gateway::payload::incoming::InteractionCreate,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{marker::CommandMarker, Id},
};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder};

use crate::interactions::application_commands::{
    ApplicationCommandData, CommonUtilities, MessageComponentData,
};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, matchmaking::MatchmakingCommandHandler,
    CommandGroupDescriptor, InteractionHandler,
};

type HandlerType = Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>;

pub struct InteractionProcessor {
    utils: Arc<CommonUtilities>,
    application_command_handlers: HashMap<Id<CommandMarker>, HandlerType>,
    component_handlers: HashMap<&'static str, HandlerType>,
    commands: Vec<Command>,
    command_groups: Vec<CommandGroupDescriptor>,
}

impl InteractionProcessor {
    pub async fn init(utils: Arc<CommonUtilities>) -> anyhow::Result<Self> {
        event!(Level::INFO, "Registering top-level command handlers");

        let top_level_handlers: Vec<Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>> = vec![
            Arc::new(Box::new(AdminCommandHandler::new(utils.clone()))),
            Arc::new(Box::new(MatchmakingCommandHandler::new(utils.clone()))),
            // Arc::new(Box::new(EulaCommandHandler::new(utils.clone()))),
            // Arc::new(Box::new(LfgCommandHandler {
            //     utils: utils.clone(),
            //     lfg_sessions,
            // })),
        ];

        let mut this = Self {
            commands: Vec::new(),
            command_groups: Vec::with_capacity(top_level_handlers.len()),
            utils,
            application_command_handlers: HashMap::new(),
            component_handlers: HashMap::new(),
        };

        this.register_commands(top_level_handlers).await?;

        Ok(this)
    }

    // Man this is so ugly
    async fn register_commands(
        &mut self,
        handlers: Vec<Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>>,
    ) -> anyhow::Result<()> {
        let groups = handlers
            .into_iter()
            .map(|h| (h.clone(), h.describe()))
            .collect::<Vec<_>>();

        self.commands = {
            let commands = groups
                .iter()
                .flat_map(|grp| {
                    grp.1
                        .commands
                        .iter()
                        .map(std::clone::Clone::clone)
                        .collect::<Vec<Command>>()
                })
                .collect::<Vec<_>>();

            if let Some(debug_guild) = crate::CONFIG.debug_guild_id {
                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .set_guild_commands(debug_guild, commands.as_slice())
                    .await?
                    .models()
                    .await?
            } else {
                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .set_global_commands(commands.as_slice())
                    .await?
                    .models()
                    .await?
            }
        };

        debug!(commands = ?serde_json::to_string(&self.commands).unwrap(), "Sent commands to discord");

        self.command_groups = groups.iter().map(|g| g.1.clone()).collect();
        for (handler, descriptor) in groups {
            debug!(desc = ?descriptor, name = ?descriptor.name, "descriptor");
            let old = self
                .component_handlers
                .insert(descriptor.name, handler.clone());

            info!(name = ?descriptor.name, "inserting command handler");

            if let Some(_) = old {
                return Err(anyhow!(
                    "tried to overwrite a component handler: {}",
                    descriptor.name
                ));
            }

            if let Some(command) = self.commands.iter().find(|c| c.name == descriptor.name) {
                if let Some(old) = self.application_command_handlers.insert(
                    command
                        .id
                        .ok_or_else(|| anyhow!("command does not have an id: {}", command.name))?,
                    handler.clone(),
                ) {
                    return Err(anyhow!(
                        "inserted a handler over a command... {:#?}",
                        old.describe().name
                    ));
                }
            } else {
                warn!(name = ?descriptor.name, "no command found");
            }
        }

        // Commands that are currently in Discord
        let global_commands = self
            .utils
            .http_client
            .interaction(self.utils.application_id)
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
                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
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
                    let data = Box::new(ApplicationCommandData {
                        command: *command.clone(),
                        id: Uuid::new_v4(),
                        interaction: interaction.0.clone(),
                        guild_id: interaction
                            .guild_id
                            .ok_or_else(|| anyhow!("you must run this command in a valid guild"))?,
                    });
                    let fut = Box::pin(Self::execute_application_command(
                        handler.clone(),
                        data,
                        self.utils.clone(),
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
        utils: Arc<CommonUtilities>,
    ) -> anyhow::Result<()> {
        utils
            .http_client
            .interaction(utils.application_id)
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
                .instrument(info_span!("command_handler")),
        );
        let res = timeout.await;
        match res {
            Ok(res) => {
                if let Err(e) = res {
                    error!(error = ?e, "Application Command Failed");
                    debug!(error = ?format!("{:?}", e), "Application Command Failed");
                    utils
                        .http_client
                        .interaction(utils.application_id)
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
                utils
                    .http_client
                    .interaction(utils.application_id)
                    .update_response(token.as_str())
                    .content(Some("Command timed out."))?
                    .await?;
                Err(anyhow!("Command timed out: {}", name))
            }
        }
        // handler.process_command(data).await
    }
}
