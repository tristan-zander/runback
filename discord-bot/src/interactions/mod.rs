pub mod application_commands;

use twilight_gateway::Shard;
use twilight_model::gateway::payload::incoming::InteractionCreate;

use crate::config::Config;

use self::application_commands::{ApplicationCommandUtilities};

pub struct InteractionHandler {
    pub application_command_utils: ApplicationCommandUtilities,
}

impl InteractionHandler {
    pub fn init(config: Config) -> Self {
        ApplicationCommandUtilities::register_all_application_commands(config);
        Self {
            application_command_utils: ApplicationCommandUtilities {},
        }
    }

    pub async fn handle_interaction<'a>(
        &'a self,
        interaction: Box<InteractionCreate>,
        _shard: &'a Shard
    ) {
        debug!(interaction = %format!("{:?}", interaction), "Received interaction");
        let _res = match &**interaction {
            // I think this is only for webhook interaction handlers
            twilight_model::application::interaction::Interaction::Ping(_) => Ok(()),
            twilight_model::application::interaction::Interaction::ApplicationCommand(command) => {
                self.application_command_utils.on_command_receive(command).await
            },
            twilight_model::application::interaction::Interaction::ApplicationCommandAutocomplete(
                _,
            ) => todo!(),
            twilight_model::application::interaction::Interaction::MessageComponent(_) => todo!(),
            _ => {
                debug!(interaction = %format!("{:?}", interaction), "Unhandled interaction");
                Ok(())
            }
        };

        // TODO: Do some error handling
    }
}
