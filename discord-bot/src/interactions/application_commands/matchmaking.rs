use twilight_model::application::command::{BaseCommandOptionData, CommandOption, CommandType};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use crate::handler;

use super::{
    ApplicationCommandHandler, ApplicationCommandUtilities, CommandDescriptor,
    CommandGroupDescriptor, InteractionData,
};

use std::sync::Arc;

pub struct MatchmakingCommandHandler;

#[async_trait]
impl ApplicationCommandHandler for MatchmakingCommandHandler {
    fn register(&self) -> CommandGroupDescriptor {
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
            commands: Box::new([CommandDescriptor {
                command,
                handler: Some(handler!(MatchmakingCommandHandler::execute)),
            }]),
        }
    }
}

impl MatchmakingCommandHandler {
    async fn execute(
        _utils: Arc<ApplicationCommandUtilities>,
        _data: Box<InteractionData>,
    ) -> anyhow::Result<()> {
        todo!()
    }
}
