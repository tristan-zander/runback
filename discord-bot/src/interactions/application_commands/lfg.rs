use std::sync::Arc;
use twilight_model::application::command::{
    ChoiceCommandOptionData, CommandOption, CommandOptionChoice, CommandType,
};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use super::{ApplicationCommandHandler, ApplicationCommandUtilities};

pub struct LfgCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl ApplicationCommandHandler for LfgCommandHandler {
    fn name(&self) -> String {
        "lfg".to_string()
    }

    fn register(&self) -> super::CommandHandlerType {
        super::CommandHandlerType::TopLevel(
            CommandBuilder::new(
                self.name(),
                "Look for games in the server".to_string(),
                CommandType::ChatInput,
            )
            .option(CommandOption::String(ChoiceCommandOptionData {
                autocomplete: false,
                choices: vec![
                    CommandOptionChoice::Int {
                        name: "15 minutes".to_string(),
                        value: 15,
                    },
                    CommandOptionChoice::Int {
                        name: "30 minutes".to_string(),
                        value: 30,
                    },
                    CommandOptionChoice::Int {
                        name: "1 hour".to_string(),
                        value: 60,
                    },
                    CommandOptionChoice::Int {
                        name: "2 hours".to_string(),
                        value: 60 * 2,
                    },
                    CommandOptionChoice::Int {
                        name: "3 hours".to_string(),
                        value: 60 * 3,
                    },
                    CommandOptionChoice::Int {
                        name: "6 hours".to_string(),
                        value: 60 * 6,
                    },
                    CommandOptionChoice::Int {
                        name: "12 hours".to_string(),
                        value: 60 * 12,
                    },
                    CommandOptionChoice::Int {
                        name: "1 day".to_string(),
                        value: 60 * 12,
                    },
                    CommandOptionChoice::Int {
                        name: "Forever (default)".to_string(),
                        value: -1,
                    },
                ],
                description: "Start/stop looking for games after a certain amount of time"
                    .to_string(),
                name: "how_long".to_string(),
                required: false,
            }))
            .option(CommandOption::String(ChoiceCommandOptionData {
                autocomplete: false,
                choices: vec![],
                description: "A short message alongside your entry".to_string(),
                name: "comment".to_string(),
                required: false,
            }))
            .validate()
            .unwrap()
            .build(),
        )
    }

    async fn execute(&self, data: &super::InteractionData) -> anyhow::Result<()> {
        todo!()
    }
}
