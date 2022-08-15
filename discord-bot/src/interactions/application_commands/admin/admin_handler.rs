use std::sync::Arc;

use twilight_model::application::command::CommandType;
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use crate::interactions::application_commands::{
    ApplicationCommandData, CommandGroupDescriptor, InteractionHandler, MessageComponentData,
};

use crate::interactions::application_commands::ApplicationCommandUtilities;

use super::{
    mm_panels_handler::MatchmakingPanelsHandler, mm_settings_handler::MatchmakingSettingsHandler,
};

pub struct AdminCommandHandler {
    // utils: Arc<ApplicationCommandUtilities>,
    matchmaking_settings_handler: MatchmakingSettingsHandler,
    matchmaking_panels_handler: MatchmakingPanelsHandler,
}

#[async_trait]
impl InteractionHandler for AdminCommandHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "admin".into(),
            "Admin configuration and management settings".into(),
            CommandType::ChatInput,
        )
        // .option(SubCommandBuilder::new(
        //     "matchmaking-panels".into(),
        //     "Add, edit, and remove matchmaking panels in your guild".into(),
        // ))
        .option(SubCommandBuilder::new(
            "matchmaking-settings".into(),
            "Shows the matchmaking settings panel".into(),
        ));

        let command = builder.build();
        CommandGroupDescriptor {
            name: "admin",
            description: "Tools for admins",
            commands: Box::new([command]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        let options = &data.command.data.options;

        if options.len() != 1 {
            return Err(anyhow!("Expected extra options when calling the top-level admin command handler. Number of arguments found: {}", options.len()));
        }

        let option = &options[0];

        let sub_command_name = options
            .get(0)
            .ok_or_else(|| anyhow!("Could not get first admin subcommand"))?
            .name
            .as_str();
        match sub_command_name {
            "matchmaking-settings" => {
                self.matchmaking_settings_handler
                    .process_command(data)
                    .await?;
            }
            "matchmaking-panels" => {
                self.matchmaking_panels_handler
                    .process_command(data)
                    .await?;
            }
            _ => {
                debug!(name = %option.name.as_str(), "Unknown admin subcommand option");
                return Err(anyhow!("Unknown admin subcommand option"));
            }
        }

        Ok(())
    }

    async fn process_autocomplete(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_modal(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!("Admin handler does not currently process modals")
    }

    async fn process_component(&self, mut data: Box<MessageComponentData>) -> anyhow::Result<()> {
        debug!(leftovers = ?data.action, "Custom ID leftovers");

        if let Some((action, field)) = data.action.split_once(':') {
            match action {
                "settings" => {
                    data.action = field.to_string();
                    self.matchmaking_settings_handler
                        .process_component(data)
                        .await?;
                    return Ok(());
                }
                "panels" => {
                    data.action = field.to_string();
                    self.matchmaking_panels_handler
                        .process_component(data)
                        .await?;
                    return Ok(());
                }
                _ => {
                    return Err(anyhow!("Unhandled action: {}", data.action));
                }
            }
        } else {
            return Err(anyhow!("Action did not match the format \"action:field\""));
        }
    }
}

impl AdminCommandHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self {
            matchmaking_settings_handler: MatchmakingSettingsHandler::new(utils.clone()),
            matchmaking_panels_handler: MatchmakingPanelsHandler::new(utils.clone()),
            // utils,
        }
    }
}
