pub mod application_commands;

use std::error::Error;

use twilight_gateway::Shard;
use twilight_model::gateway::payload::incoming::InteractionCreate;

use crate::config::Config;

use self::application_commands::ApplicationCommandUtilities;

pub struct InteractionHandler {
    pub application_command_utils: ApplicationCommandUtilities,
}

impl InteractionHandler {
    pub async fn init(config: &Config) -> Result<Self, Box<dyn Error>> {
        let application_command_utils = ApplicationCommandUtilities::new(config).await?;
        application_command_utils
            .register_all_application_commands(config.debug_guild_id)
            .await?;
        Ok(Self {
            application_command_utils,
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
        if let Err(e) = res {
            error!(error = %e, "Unhandled interaction error");
        }
    }
}
