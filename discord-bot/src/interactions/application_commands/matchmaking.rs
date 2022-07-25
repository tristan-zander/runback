use twilight_model::application::command::{BaseCommandOptionData, CommandOption, CommandType};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use super::{
    ApplicationCommandUtilities, CommandGroupDescriptor, InteractionData, InteractionHandler,
};

use std::sync::Arc;

pub struct MatchmakingCommandHandler;

#[async_trait]
impl InteractionHandler for MatchmakingCommandHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "matchmaking".to_string(),
            "Matchmaking commands".to_string(),
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new(
                "play-against".to_string(),
                "Start a match with an opponent".to_string(),
            )
            .option(CommandOption::User(BaseCommandOptionData {
                name: "opponent".to_string(),
                description: "The user that you wish to play against".to_string(),
                description_localizations: None,
                name_localizations: None,
                required: true,
            }))
            .build(),
        );
        // .option(
        //     SubCommandBuilder::new("show-matches".into(), "Show the matchmaking menu".into())
        //         .build(),
        // )
        // .option(
        //     SubCommandBuilder::new(
        //         "settings".into(),
        //         "View and update settings such as default character".into(),
        //     )
        //     .build(),
        // )
        // .option(
        //     SubCommandBuilder::new(
        //         "end-session".into(),
        //         "Finish your matchmaking session".into(),
        //     )
        //     .build(),
        // );

        let command = builder.build();
        CommandGroupDescriptor {
            name: "matchmaking",
            description: "Commands that are related to matchmaking",
            commands: Box::new([command]),
        }
    }

    async fn process_command(&self, data: Box<InteractionData>) -> anyhow::Result<()> {
        todo!("Command processor not initiated")
    }

    async fn process_autocomplete(&self, _data: Box<InteractionData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_modal(&self, _data: Box<InteractionData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_component(&self, _data: Box<InteractionData>) -> anyhow::Result<()> {
        unreachable!()
    }
}
