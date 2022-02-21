pub mod application_commands;

use std::error::Error;

use twilight_gateway::Shard;
use twilight_model::gateway::payload::incoming::InteractionCreate;

use crate::config::Config;

use self::application_commands::{ApplicationCommandHandlers, ApplicationCommandUtilities};

pub struct InteractionHandler {
    application_command_handlers: ApplicationCommandHandlers,
}

impl InteractionHandler {
    pub async fn init() -> Result<Self, Box<dyn Error>> {
        let application_command_handlers = ApplicationCommandHandlers::new().await?;
        application_command_handlers.utilities.register_all_application_commands().await?;
        Ok(Self {
            application_command_handlers,
        })
    }

    pub async fn handle_interaction<'a>(
        &self,
        interaction: Box<InteractionCreate>,
        _shard: &'a Shard,
    ) {
        debug!(interaction = %format!("{:?}", interaction), "Received interaction");
        let res = match &**interaction {
            // I think this is only for webhook interaction handlers
            twilight_model::application::interaction::Interaction::Ping(_) => Ok(()),
            twilight_model::application::interaction::Interaction::ApplicationCommand(command) => {
                self.application_command_handlers.on_command_receive(command).await
            },
            // twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
            //     _,
            // ) => todo!(),
            // twilight_model::application::interaction::Interaction::MessageComponent(_) => todo!(),
            _ => {
                debug!(interaction = %format!("{:?}", interaction), "Unhandled interaction");
                Ok(())
            }
        };

        // TODO: Do some error handling
        if let Err(e) = res {
            error!(error = %e, "Unhandled interaction error");
        }
    }
}
