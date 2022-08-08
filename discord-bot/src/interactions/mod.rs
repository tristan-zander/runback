pub mod application_commands;
pub mod panels;

use std::{collections::HashMap, sync::Arc, time::Duration};

use entity::sea_orm::{prelude::Uuid, DatabaseConnection};

use futures::future::BoxFuture;
use tokio::time::timeout;
use tracing::{Instrument, Level};
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::Shard;
use twilight_model::{
    application::{command::Command, interaction::Interaction},
    channel::message::MessageFlags,
    gateway::payload::incoming::InteractionCreate,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{marker::CommandMarker, Id},
};
use twilight_standby::Standby;
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder};

use crate::interactions::application_commands::{
    ApplicationCommandData, ApplicationCommandUtilities, MessageComponentData,
};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, eula::EulaCommandHandler,
    matchmaking::MatchmakingCommandHandler, CommandGroupDescriptor, InteractionHandler,
    PingCommandHandler,
};

type HandlerType = Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>;

pub struct InteractionProcessor {
    utils: Arc<ApplicationCommandUtilities>,
    application_command_handlers: HashMap<Id<CommandMarker>, HandlerType>,
    component_handlers: HashMap<&'static str, HandlerType>,
    commands: Vec<Command>,
    command_groups: Vec<CommandGroupDescriptor>,
}

impl InteractionProcessor {
    pub async fn init(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> anyhow::Result<Self> {
        let utils = Arc::new(ApplicationCommandUtilities::new(db, cache, standby).await?);
        // let lfg_sessions = Arc::new(DashMap::new());

        event!(Level::INFO, "Registering top-level command handlers");

        let top_level_handlers: Vec<Arc<Box<dyn InteractionHandler + Send + Sync + 'static>>> = vec![
            Arc::new(Box::new(PingCommandHandler {
                utils: utils.clone(),
            })),
            Arc::new(Box::new(AdminCommandHandler::new(utils.clone()))),
            Arc::new(Box::new(MatchmakingCommandHandler::new(utils.clone()))),
            Arc::new(Box::new(EulaCommandHandler::new(utils.clone()))),
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
                        .into_iter()
                        .map(|c| c.to_owned())
                        .collect::<Vec<Command>>()
                })
                .collect::<Vec<_>>();

            self.utils
                .http_client
                .interaction(self.utils.application_id)
                .set_guild_commands(crate::CONFIG.debug_guild_id.unwrap(), commands.as_slice())
                .exec()
                .await?
                .models()
                .await?
        };

        debug!(commands = ?serde_json::to_string(&self.commands).unwrap(), "Sent commands to discord");

        self.command_groups = groups.iter().map(|g| g.1.clone()).collect();
        for (handler, descriptor) in groups {
            debug!(desc = ?descriptor, "Descriptor");
            let old = self
                .component_handlers
                .insert(descriptor.name, handler.clone());

            if let Some(_) = old {
                return Err(anyhow!(
                    "Tried to overwrite a component handler: {}",
                    descriptor.name
                ));
            }

            for c in self.commands.iter() {
                if c.id.is_none()
                    || descriptor
                        .commands
                        .into_iter()
                        .find(|desc_comm| desc_comm.name == c.name)
                        .is_none()
                {
                    debug!(command = ?c.name, "Command does not have an id");
                    continue;
                }

                if let Some(old) = self.application_command_handlers.insert(
                    c.id.ok_or_else(|| anyhow!("Command does not have an id: {}", c.name))?,
                    handler.clone(),
                ) {
                    return Err(anyhow!(
                        "Inserted a handler over a command... {:#?}",
                        old.describe().name
                    ));
                }
            }
        }

        Ok(())
    }

    pub fn handle_interaction<'shard>(
        &self,
        interaction: InteractionCreate,
        _shard: &'shard Shard,
    ) -> anyhow::Result<BoxFuture<'static, anyhow::Result<()>>> {
        event!(tracing::Level::DEBUG, "Received interaction");

        // TODO: Send a deferred message response, followup on it later

        match &*interaction {
            // I think this is only for webhook interaction handlers
            // twilight_model::application::interaction::Interaction::Ping(_) => ,
            Interaction::ApplicationCommand(command) => {
                // self.application_command_handlers
                //     .on_command_receive(command.as_ref())
                //     .await?;
                if let Some(handler) = self.application_command_handlers.get(&command.data.id) {
                    let data = Box::new(ApplicationCommandData {
                        command: *command.to_owned(),
                        id: Uuid::new_v4(),
                    });
                    let handler = handler.clone();
                    let utils = self.utils.clone();
                    let fut = Box::pin(
                        async move {
                            utils.http_client.interaction(utils.application_id).create_response(data.command.id, data.command.token.as_str(), &InteractionResponse { kind: InteractionResponseType::DeferredChannelMessageWithSource, data: None }).exec().await?;
                            let name = data.command.data.name.clone();
                            let token = data.command.token.clone();
                            let runback_id = data.id.clone();
                            let timeout = timeout(Duration::from_secs(5), handler.process_command(data).instrument(info_span!("command_handler")));
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
                                            .content(format!("```\n{}\n```", e).as_str())?
                                            .embeds(&[
                                                EmbedBuilder::new()
                                                    .description("An error has occurred.")
                                                    .footer(EmbedFooterBuilder::new(runback_id.to_hyphenated_ref().to_string()).build())
                                                    .field(
                                                        EmbedFieldBuilder::new("error", e.to_string()).build()
                                                    )
                                                    .validate()?
                                                    .build()
                                            ])?
                                            .exec()
                                            .await?;
                                    }

                                    return Ok(());
                                },
                                Err(_) => {
                                    utils.http_client.interaction(utils.application_id).update_response(token.as_str()).content(Some("Command timed out."))?.exec().await?;
                                    return Err(anyhow!("Command timed out: {}", name));
                                },
                            }
                            // handler.process_command(data).await 
                        });
                    return Ok(fut);
                } else {
                    error!(name = ?command.data.name,"No command handler found");
                    return Err(anyhow!("No such command handler found"));
                }
            }
            twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
                _,
            ) => debug!("Received autocomplete"),
            Interaction::MessageComponent(message) => {
                debug!("Received message component");

                if let Some((handler_name, leftover)) = message.data.custom_id.split_once(':') {
                    if let Some(handler) = self.component_handlers.get(handler_name) {
                        let data = Box::new(MessageComponentData {
                            id:Uuid::new_v4(), message: *message.to_owned(), action: leftover.to_string() }
                        );
                        let handler = handler.clone();
                        let fut = Box::pin(async move { handler.process_component(data).await });
                        return Ok(fut);
                    } else {
                        return Err(anyhow!("Invalid message component handler: {}", handler_name))
                    }
                } else {
                    return Err(anyhow!("Message component custom_id does not match the format \"handler:action\""));
                }
            }
            Interaction::ModalSubmit(_modal) => {
                debug!("Received modal");
            }
            _ => {
                debug!(interaction = %format!("{:?}", interaction), "Unhandled interaction");
            }
        }

        trace!("Ended interaction response.");
        return Err(anyhow!("Interaction was unmatched"));
    }
}
