use twilight_model::{
    application::command::{Command, CommandType},
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use super::{ApplicationCommandHandler, CommandHandlerType};

pub struct MatchmakingCommandHandler;

#[async_trait]
impl ApplicationCommandHandler for MatchmakingCommandHandler {
    fn register(&self) -> CommandHandlerType {
        let builder = CommandBuilder::new(
            self.name(),
            "Matchmaking commands".into(),
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new("show-matches".into(), "Show the matchmaking menu".into())
                .build(),
        )
        .option(
            SubCommandBuilder::new(
                "settings".into(),
                "View and update settings such as default character".into(),
            )
            .build(),
        )
        .option(
            SubCommandBuilder::new(
                "end-session".into(),
                "Finish your matchmaking session".into(),
            )
            .build(),
        );

        let comm = builder.build();
        return CommandHandlerType::TopLevel(comm);
    }

    async fn execute(&self, data: &super::InteractionData) -> anyhow::Result<()> {
        unimplemented!()
    }

    fn name(&self) -> String {
        "matchmaking".into()
    }
}
