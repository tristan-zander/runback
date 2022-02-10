use std::{collections::HashMap, error::Error};

use twilight_http::Client as HttpClient;
use twilight_model::{
    application::command::{Command, CommandOption, CommandType},
    gateway::payload::incoming::InteractionCreate,
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder, SubCommandBuilder};

use crate::config::Config;

pub async fn register_all_application_commands(
    config: Config,
) -> Result<HashMap<String, Box<dyn ApplicationCommandCallback>>, Box<dyn Error>> {
    let debug_guild = config.debug_guild_id;

    let mut command_map: HashMap<String, Box<dyn ApplicationCommandCallback>> = HashMap::new();
    let mut commands = vec![
        PingCommandHander::to_command(debug_guild),
        MatchmakingCommandHandler::to_command(debug_guild),
    ];

    // Sorry that this is really ugly
    command_map.insert(commands[0].name.clone(), Box::new(PingCommandHander {}));

    command_map.insert(
        commands[1].name.clone(),
        Box::new(MatchmakingCommandHandler {
            command: commands[1].clone(),
        }),
    );

    let client = HttpClient::new(config.token);
    let application_id = {
        let response = client.current_user_application().exec().await?;
        response.model().await?.id
    };

    let res = client
        .interaction(application_id)
        .set_guild_commands(config.debug_guild_id.unwrap(), commands.as_slice())
        .exec()
        .await?
        .models()
        .await?;

    debug!(res = %format!("{:?}", res), "Successfully set guild commands");

    Ok(command_map)
}

pub trait ApplicationCommand {
    /// Return the command in a form that can be registered by Discord through an http call.
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command;
}

/// Each Application Command handler must implement this trait so it can be turned into registered and executed.
pub trait ApplicationCommandCallback {
    /// Execute the command at runtime.
    fn execute(&mut self, data: Box<InteractionCreate>) -> Result<(), Box<dyn Error>>;
}

struct PingCommandHander;

impl ApplicationCommand for PingCommandHander {
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command {
        let mut builder = CommandBuilder::new(
            "ping".into(),
            "Responds with pong".into(),
            CommandType::ChatInput,
        )
        .option(StringBuilder::new(
            "text".into(),
            "Send this text alongside the response".into(),
        ));

        if let Some(id) = debug_guild {
            builder = builder.guild_id(id);
        }

        let comm = builder.build();
        debug!(comm = %format!("{:?}", comm), "Created command");
        return comm;
    }
}

impl ApplicationCommandCallback for PingCommandHander {
    fn execute(&mut self, _data: Box<InteractionCreate>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}

struct MatchmakingCommandHandler {
    command: Command,
}

impl ApplicationCommand for MatchmakingCommandHandler {
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command {
        let mut builder = CommandBuilder::new(
            "matchmaking".into(),
            "Matchmaking related commands".into(),
            CommandType::ChatInput,
        )
        .option(
            SubCommandBuilder::new("start".into(), "Start a matchmaking session".into()).build(),
        )
        .option(
            SubCommandBuilder::new("end".into(), "Finish a matchmaking session".into()).build(),
        );

        if let Some(id) = debug_guild {
            builder = builder.guild_id(id);
        }

        let comm = builder.build();
        debug!(comm = %format!("{:?}", comm), "Created command!");
        return comm;
    }
}

impl ApplicationCommandCallback for MatchmakingCommandHandler {
    fn execute(&mut self, _data: Box<InteractionCreate>) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
}
