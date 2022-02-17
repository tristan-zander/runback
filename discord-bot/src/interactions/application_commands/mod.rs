mod eula;

use std::error::Error;

use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_http::Client as DiscordHttpClient;
use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::{Command, CommandType},
        interaction::ApplicationCommand as DiscordApplicationCommand,
    },
    channel::message::MessageFlags,
    id::{
        marker::{ApplicationMarker, GuildMarker},
        Id,
    },
};
use twilight_util::builder::{
    command::{
        ChannelBuilder, CommandBuilder, RoleBuilder, StringBuilder, SubCommandBuilder,
        SubCommandGroupBuilder,
    },
    CallbackDataBuilder,
};

use crate::config::Config;

use self::eula::EULACommandHandler;

pub struct ApplicationCommandUtilities {
    http_client: DiscordHttpClient,
    application_id: Id<ApplicationMarker>,
    eula_command_handler: EULACommandHandler,
}

impl ApplicationCommandUtilities {
    pub async fn new(config: &Config) -> Result<Self, Box<dyn Error>> {
        let http_client = DiscordHttpClient::new(config.token.clone());
        let application_id = {
            let response = http_client.current_user_application().exec().await?;
            response.model().await?.id
        };

        Ok(Self {
            http_client,
            application_id,
            eula_command_handler: EULACommandHandler {
                http_client: DiscordHttpClient::new(config.token.clone()),
                application_id,
            },
        })
    }

    #[allow(dead_code)]
    pub fn new_with_application_id(config: &Config, application_id: Id<ApplicationMarker>) -> Self {
        Self {
            http_client: DiscordHttpClient::new(config.token.clone()),
            application_id,
            eula_command_handler: EULACommandHandler {
                http_client: DiscordHttpClient::new(config.token.clone()),
                application_id,
            },
        }
    }

    pub async fn register_all_application_commands(
        &self,
        debug_guild: Option<Id<GuildMarker>>,
    ) -> Result<(), Box<dyn Error>> {
        let commands = vec![
            PingCommandHander::to_command(debug_guild),
            MatchmakingCommandHandler::to_command(debug_guild),
            AdminCommandHandler::to_command(debug_guild),
            EULACommandHandler::to_command(debug_guild),
        ];

        // TODO: In the future, only set as guild commands if we're running in production mode or the debug_guild is empty
        let res = self
            .http_client
            .interaction(self.application_id)
            .set_guild_commands(debug_guild.unwrap(), commands.as_slice())
            .exec()
            .await?
            .models()
            .await?;

        debug!(commands = %format!("{:?}", res), "Successfully set guild commands");

        Ok(())
    }

    pub async fn on_command_receive(
        &self,
        command: &Box<DiscordApplicationCommand>,
    ) -> Result<(), Box<dyn Error>> {
        // TODO: Assert that the guild has accepted the EULA
        // if has_accepted_eula(command.guild_id) {
        // Send a message to the user, saying that a server administrator needs to accept the eula
        // }

        let _command_id = command.data.id;
        let command_name = command.data.name.as_str();
        match command_name {
            "ping" => {
                // Respond with `Pong` with an ephemeral message and the current ping in ms

                let message = InteractionResponse::ChannelMessageWithSource(
                    CallbackDataBuilder::new()
                        .embeds(vec![EmbedBuilder::new()
                            .color(0x55_4e_2b)
                            .description("Runback Matchmaking Bot")
                            .field(EmbedFieldBuilder::new("Ping?", "Pong!").build())
                            .build()?])
                        .flags(MessageFlags::EPHEMERAL)
                        .build(),
                );

                let _res = self
                    .http_client
                    .interaction(self.application_id)
                    .interaction_callback(command.id, command.token.as_str(), &message)
                    .exec()
                    .await?;

                debug!(message = %format!("{:?}", message), "Reponded to command \"Pong\"");

                // self.client.message(command.channel_id, message_id);
            }
            "eula" => {
                // Send a message with the EULA as the message body (or a link to the website)
                self.eula_command_handler.on_command_called(command).await?;
            }
            "mm" => {
                // Find the related matchmaking subcommand
            }
            "league" => {
                // Find the related league subcommand
            }
            "tournament" => {
                // Find the related tournament subcommand
            }
            "admin" => {
                // Admin related settings
            }
            _ => debug!(command_name = %command_name, "Unhandled application command"),
        }

        Ok(())
    }
}

// TODO: This should definitely be renamed to something else so it doesn't conflict with twilight_models
pub trait ApplicationCommand {
    /// Return the command in a form that can be registered by Discord through an http call.
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command;
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

struct MatchmakingCommandHandler;

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

struct AdminCommandHandler;

impl ApplicationCommand for AdminCommandHandler {
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command {
        let mut builder = CommandBuilder::new(
            "admin".into(),
            "Admin configuration and management settings".into(),
            CommandType::ChatInput,
        )
        .option(
            SubCommandGroupBuilder::new("matchmaking".into(), "Matchmaking settings".into())
                .subcommands(vec![
                    SubCommandBuilder::new(
                        "set-matchmaking-channel".into(),
                        "Set the channel where the matchmaking panel will be posted".into(),
                    )
                    .option(ChannelBuilder::new(
                        "channel".into(),
                        "The channel that the matchmaking panel will be posted".into(),
                    )),
                    SubCommandBuilder::new(
                        "set-matchmaking-role".into(),
                        "Set which role can access matchmaking".into(),
                    )
                    .option(RoleBuilder::new(
                        "matchmaking-role".into(),
                        "The role that can access matchmaking features".into(),
                    )),
                ]),
        );

        if let Some(id) = debug_guild {
            builder = builder.guild_id(id);
        }

        let comm = builder.build();
        debug!(command = %format!("{:?}", comm), "Created command!");
        return comm;
    }
}
