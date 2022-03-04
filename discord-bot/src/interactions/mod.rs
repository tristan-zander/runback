pub mod application_commands;

use std::{error::Error, sync::Arc};

use entity::sea_orm::DatabaseConnection;
use lazy_static::__Deref;
use tracing::Instrument;
use twilight_gateway::Shard;
use twilight_model::{
    application::callback::InteractionResponse, channel::message::MessageFlags,
    gateway::payload::incoming::InteractionCreate,
};
use twilight_util::builder::CallbackDataBuilder;

use crate::{config::Config, RunbackError};

use self::application_commands::{ApplicationCommandHandlers, ApplicationCommandUtilities};

pub struct InteractionHandler {
    application_command_handlers: ApplicationCommandHandlers,
}

impl InteractionHandler {
    pub async fn init(db: Arc<Box<DatabaseConnection>>) -> Result<Self, Box<dyn Error>> {
        let application_command_handlers = ApplicationCommandHandlers::new(db).await?;
        application_command_handlers
            .utilities
            .register_all_application_commands()
            .await?;
        Ok(Self {
            application_command_handlers,
        })
    }

    pub async fn handle_interaction<'shard>(
        &self,
        interaction: Box<InteractionCreate>,
        _shard: &'shard Shard,
    ) -> Result<(), RunbackError> {
        event!(tracing::Level::DEBUG, "Received interaction");

        // TODO: Send a deferred message response, followup on it later

        match &**interaction {
            // I think this is only for webhook interaction handlers
            // twilight_model::application::interaction::Interaction::Ping(_) => ,
            twilight_model::application::interaction::Interaction::ApplicationCommand(command) => {
                self.application_command_handlers
                    .on_command_receive(command.deref())
                    .instrument(debug_span!("interaction::application_command"))
                    .await?;
            }
            // twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
            //     _,
            // ) => todo!(),
            twilight_model::application::interaction::Interaction::MessageComponent(m) => {
                self.application_command_handlers
                    .on_message_component_event(m.deref())
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
