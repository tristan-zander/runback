use twilight_model::{id::{Id, marker::GuildMarker}, application::command::{Command, CommandType}};
use twilight_util::builder::command::{CommandBuilder, SubCommandBuilder};

use super::ApplicationCommand;

pub(super) struct MatchmakingCommandHandler;

impl ApplicationCommand for MatchmakingCommandHandler {
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command {
        let mut builder = CommandBuilder::new(
            "mm".into(),
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

        if let Some(id) = debug_guild {
            builder = builder.guild_id(id);
        }

        let comm = builder.build();
        debug!(command = %format!("{:?}", comm), "Created command!");
        return comm;
    }
}

