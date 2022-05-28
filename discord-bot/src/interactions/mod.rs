pub mod application_commands;
pub mod panels;

use std::{collections::HashMap, sync::Arc};

use entity::sea_orm::DatabaseConnection;

use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::Shard;
use twilight_model::{
    application::command::Command, gateway::payload::incoming::InteractionCreate,
};
use twilight_standby::Standby;

use crate::{error::RunbackError, interactions::application_commands::InteractionData};

use self::application_commands::{
    admin::AdminCommandHandler, eula::EulaCommandHandler, matchmaking::MatchmakingCommandHandler,
    ApplicationCommandHandler, ApplicationCommandHandlers, PingCommandHandler,
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

        let mut new = Self {
            application_command_handlers,
            command_map: HashMap::new(),
        };

        let top_level_handlers: Vec<Box<dyn ApplicationCommandHandler + Send + Sync>> = vec![
            Box::new(PingCommandHandler {
                utils: new.application_command_handlers.utils.clone(),
            }),
            Box::new(AdminCommandHandler::new(
                new.application_command_handlers.utils.clone(),
            )),
            Box::new(MatchmakingCommandHandler {}),
            Box::new(EulaCommandHandler::new(
                new.application_command_handlers.utils.clone(),
            )),
        ];

        let mut command_models = Vec::new();

        for handler in top_level_handlers {
            if let Some(ref mut c) = handler.register() {
                c.guild_id = crate::CONFIG.debug_guild_id;

                command_models.push(c.to_owned());

                new.command_map.insert(c.name.to_owned(), handler);
            }
        }

        let res = new
            .application_command_handlers
            .utils
            .http_client
            .interaction(new.application_command_handlers.utils.application_id)
            .set_guild_commands(
                crate::CONFIG.debug_guild_id.unwrap(),
                command_models.as_slice(),
            )
            .exec()
            .await?
            .models()
            .await?;

        Ok(new)
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
