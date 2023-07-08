pub mod application_commands;
pub mod panels;

use std::{collections::HashMap, sync::Arc, time::Duration};

use crate::client::RunbackClient;

use futures::future::BoxFuture;
use serde::Serialize;
use tokio::time::timeout;
use tracing::{Instrument, Level};
use twilight_gateway::Shard;
use twilight_model::{
    application::interaction::InteractionData,
    gateway::payload::incoming::InteractionCreate,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{marker::CommandMarker, Id},
};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder};

use crate::{client::DiscordClient, interactions::application_commands::ApplicationCommandData};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, matchmaking::MatchmakingCommandHandler,
    CommandGroupDescriptor, InteractionHandler,
};

type HandlerType = Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>;

#[derive(Serialize)]
struct HandlerDetails {
    #[serde(skip_serializing)]
    pub handler: HandlerType,
    pub describe: CommandGroupDescriptor,
}

pub struct InteractionProcessor {
    handlers: HashMap<Id<CommandMarker>, Arc<HandlerDetails>>,
    discord_client: DiscordClient,
}

impl InteractionProcessor {
    pub fn new(discord_client: DiscordClient) -> Self {
        Self {
            discord_client,
            handlers: Default::default(),
        }
    }

    #[instrument(name = "init interaction processor", skip_all)]
    pub async fn init(&mut self, client: &RunbackClient) -> anyhow::Result<()> {
        event!(Level::INFO, "Registering top-level command handlers");

        let handlers = vec![
            self.prepare_handler::<AdminCommandHandler>(client),
            self.prepare_handler::<MatchmakingCommandHandler>(client),
        ];

        self.register_commands(handlers).await?;

        Ok(())
    }

    fn prepare_handler<T: InteractionHandler + Send + Sync + 'static>(
        &mut self,
        client: &RunbackClient,
    ) -> HandlerDetails {
        let describe = T::describe();
        debug!(handler = ?describe.name, "registering handler");

        let handler: Arc<Box<(dyn InteractionHandler + std::marker::Send + Sync + 'static)>> =
            Arc::new(Box::new(T::create(client)));

        HandlerDetails { handler, describe }
    }

    async fn register_commands<T: IntoIterator<Item = HandlerDetails>>(
        &mut self,
        handlers: T,
    ) -> anyhow::Result<()> {
        let handlers = handlers
            .into_iter()
            .map(|h| Arc::new(h))
            .collect::<Vec<_>>();

        let name_to_handler_mapping = handlers
            .iter()
            .flat_map(|h| {
                h.describe
                    .commands
                    .iter()
                    .map(|c| (c.name.as_str(), h.clone()))
            })
            .collect::<HashMap<_, _>>();

        debug!(
            "{}",
            serde_json::to_string_pretty(&name_to_handler_mapping)?
        );
        // debug!(val = ?serde_json::dese);

        let commands = {
            let commands = handlers
                .iter()
                .flat_map(|d| d.describe.commands.as_ref().to_owned())
                .collect::<Vec<_>>();

            if let Some(debug_guild) = crate::CONFIG.debug_guild_id {
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

        for command in commands {
            let cid = command
                .id
                .ok_or_else(|| anyhow!("could not get command id"))?;
            if let Some(handler) = name_to_handler_mapping.get(&command.name.as_str()) {
                if let Some(old) = self.handlers.insert(cid, handler.clone()) {
                    warn!(new = ?handler.describe.name, old = ?old.describe.name, command = ?command.name, "commands have conflicting names");
                }
            } else {
                error!(name = ?command.name, "no handler exists with given name");
            }
        }

        event!(Level::INFO, "sent commands to discord");

        // Commands that are currently in Discord
        let global_commands = self
            .discord_client
            .interaction()
            .global_commands()
            .await?
            .models()
            .await?;

        for c in global_commands {
            let cid =
                c.id.ok_or_else(|| anyhow!("command sent from discord has no id"))?;

            if !self.handlers.contains_key(&cid) {
                warn!(name = ?c.name, id = ?c.id, "unknown global command found on Discord, will delete");
                self.discord_client
                    .interaction()
                    .delete_global_command(cid) // realistically, this error will never be shown
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
                // TODO: Send a deferred message response, followup on it later
                if let Some(handler_wrapper) = self.handlers.get(&command.id) {
                    let data = Box::new(ApplicationCommandData::new(
                        *command.clone(),
                        interaction.0.clone(),
                    )?);

                    let fut = Box::pin(Self::execute_application_command(
                        handler_wrapper.handler.clone(),
                        data,
                        self.discord_client.clone(),
                    ));
                    return Ok(fut);
                } else {
                    error!(name = ?command.name,"No command handler found");
                    return Err(anyhow!("No such command handler found"));
                }
            }
            InteractionData::MessageComponent(_message) => {
                debug!("Received message component");

                todo!();
                // if let Some((handler_name, leftover)) = message.custom_id.split_once(':') {
                //     if let Some(handler) = self.component_handlers.get(handler_name) {
                //         let data = Box::new(MessageComponentData {
                //             id: Uuid::new_v4(),
                //             message: message.clone(),
                //             action: leftover.to_string(),
                //             interaction: interaction.0.clone(),
                //         });
                //         let handler = handler.clone();
                //         let fut = Box::pin(async move { handler.process_component(data).await });
                //         return Ok(fut);
                //     } else {
                //         return Err(anyhow!(
                //             "Invalid message component handler: {}",
                //             handler_name
                //         ));
                //     }
                // } else {
                //     return Err(anyhow!(
                //         "Message component custom_id does not match the format \"handler:action\""
                //     ));
                // }
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
            Duration::from_secs(10),
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
                    let _msg = discord_client
                        .send_error_response(
                            token.as_str(),
                            runback_id,
                            e.to_string().as_str(),
                        )
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
