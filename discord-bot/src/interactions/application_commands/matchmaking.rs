use twilight_model::{
    application::command::{Command, CommandType},
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use super::ApplicationCommandHandler;

pub struct MatchmakingCommandHandler;

#[async_trait]
impl ApplicationCommandHandler for MatchmakingCommandHandler {
    fn register(&self) -> Option<Command> {
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
        debug!(command = %format!("{:?}", comm), "Created command!");
        return Some(comm);
    }

    async fn execute(&self, data: &super::InteractionData) -> anyhow::Result<()> {
        Err(anyhow!("Unimplemented"))
    }

    fn name(&self) -> String {
        "mm".into()
    }
}
