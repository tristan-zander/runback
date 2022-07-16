pub mod application_commands;
pub mod panels;

use std::{collections::HashMap, sync::Arc};

use dashmap::DashMap;
use entity::sea_orm::DatabaseConnection;

use tracing::Level;
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::Shard;
use twilight_model::{
    gateway::payload::incoming::InteractionCreate,
};
use twilight_standby::Standby;

use crate::{
    error::RunbackError,
    interactions::application_commands::{lfg::LfgCommandHandler, InteractionData},
};

use self::application_commands::{
    admin::admin_handler::AdminCommandHandler, eula::EulaCommandHandler,
    matchmaking::MatchmakingCommandHandler, ApplicationCommandHandler, ApplicationCommandHandlers,
    CommandHandlerType, PingCommandHandler,
};

pub struct InteractionHandler {
    application_command_handlers: ApplicationCommandHandlers,
    /// The name of the command, then the Command Handler associated with that command.
    /// ApplicationCommandHandlers with any SubCommands or SubCommandGroups will also have this structure
    command_map: HashMap<String, Box<dyn ApplicationCommandHandler + Sync + Send + 'static>>,
}

impl InteractionHandler {
    pub async fn init(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> Result<Self, RunbackError> {
        let application_command_handlers =
            ApplicationCommandHandlers::new(db, cache, standby).await?;

        let mut command_map = HashMap::new();

        let lfg_sessions = Arc::new(DashMap::new());

        event!(Level::INFO, "Registering top-level command handlers");

        let top_level_handlers: Vec<Box<dyn ApplicationCommandHandler + Send + Sync>> = vec![
            Box::new(PingCommandHandler {
                utils: application_command_handlers.utils.clone(),
            }),
            Box::new(AdminCommandHandler::new(
                application_command_handlers.utils.clone(),
            )),
            Box::new(MatchmakingCommandHandler {}),
            Box::new(EulaCommandHandler::new(
                application_command_handlers.utils.clone(),
            )),
            Box::new(LfgCommandHandler {
                utils: application_command_handlers.utils.clone(),
                lfg_sessions,
            }),
        ];

        let mut command_models = Vec::new();

        for handler in top_level_handlers {
            if let CommandHandlerType::TopLevel(ref mut c) = handler.register() {
                c.guild_id = crate::CONFIG.debug_guild_id;

                command_models.push(c.to_owned());

                command_map.insert(c.name.to_owned(), handler);

                debug!(name = %c.name, "Registered application command handler");
            }
        }

        let _res = application_command_handlers
            .utils
            .http_client
            .interaction(application_command_handlers.utils.application_id)
            .set_guild_commands(
                crate::CONFIG.debug_guild_id.unwrap(),
                command_models.as_slice(),
            )
            .exec()
            .await?
            .models()
            .await?;

        Ok(Self {
            application_command_handlers,
            command_map,
        })
    }

    #[tracing::instrument(skip_all)]
    pub async fn handle_interaction<'shard>(
        &self,
        interaction: InteractionCreate,
        _shard: &'shard Shard,
    ) -> Result<(), RunbackError> {
        event!(tracing::Level::DEBUG, "Received interaction");

        // TODO: Send a deferred message response, followup on it later

        match &*interaction {
            // I think this is only for webhook interaction handlers
            // twilight_model::application::interaction::Interaction::Ping(_) => ,
            twilight_model::application::interaction::Interaction::ApplicationCommand(command) => {
                // self.application_command_handlers
                //     .on_command_receive(command.as_ref())
                //     .await?;
                let name: &str = command.data.name.as_ref();

                if let Some(handler) = self.command_map.get(name) {
                    handler
                        .execute(&InteractionData {
                            command: command.as_ref(),
                        })
                        .await
                        .map_err(|e| RunbackError {
                            message: "Application Command error".to_string(),
                            inner: Some(e.into()),
                        })?;
                } else {
                    error!("No command found");
                    return Err(RunbackError {
                        message: "No such command found".into(),
                        inner: None,
                    });
                }
            }
            // twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
            //     _,
            // ) => todo!(),
            twilight_model::application::interaction::Interaction::MessageComponent(message) => {
                self.application_command_handlers
                    .on_message_component_event(message.as_ref())
                    .await?;
            }
            twilight_model::application::interaction::Interaction::ModalSubmit(modal) => {
                debug!("Received modal");
                self.application_command_handlers
                    .on_modal_submit(modal.as_ref())
                    .await?;
            }
            _ => {
                debug!(interaction = %format!("{:?}", interaction), "Unhandled interaction");
            }
        }

        // TODO: Do some error handling, send a message back to the user
        // if let Err(e) = res {
        //     error!(error = %e, "Unhandled interaction error");
        // }

        debug!("Ended interaction response.");
        Ok(())
    }
}
