mod admin;
mod eula;
mod matchmaking;

use std::{error::Error, sync::Arc};

use entity::sea_orm::DatabaseConnection;
use twilight_embed_builder::{EmbedBuilder, EmbedFieldBuilder};
use twilight_http::Client as DiscordHttpClient;
use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::{Command, CommandType},
        interaction::{
            ApplicationCommand as DiscordApplicationCommand, MessageComponentInteraction,
        },
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

use crate::{
    config::Config, interactions::application_commands::matchmaking::MatchmakingCommandHandler,
};

use self::{admin::AdminCommandHandler, eula::EULACommandHandler};

/// Contains any helper functions to help make writing application command handlers easier
// MAKE SURE THIS IS THREAD SAFE AND USABLE WITHOUT A MUTEX!!
pub struct ApplicationCommandUtilities {
    pub http_client: DiscordHttpClient,
    pub application_id: Id<ApplicationMarker>,
    pub db: Arc<Box<DatabaseConnection>>,
}

pub struct ApplicationCommandHandlers {
    pub utilities: Arc<ApplicationCommandUtilities>,
    eula_command_handler: EULACommandHandler,
    admin_command_handler: AdminCommandHandler,
}

impl ApplicationCommandHandlers {
    pub async fn new(db: Arc<Box<DatabaseConnection>>) -> Result<Self, Box<dyn Error>> {
        let utilities = Arc::new(ApplicationCommandUtilities::new(db).await?);
        Ok(Self {
            utilities: utilities.clone(),
            eula_command_handler: EULACommandHandler::new(utilities.clone()),
            admin_command_handler: AdminCommandHandler::new(utilities.clone()),
        })
    }

    pub async fn on_command_receive(
        &self,
        command: &Box<DiscordApplicationCommand>,
    ) -> Result<(), Box<dyn Error>> {
        // TODO: Assert that the guild has accepted the EULA
        // if has_accepted_eula(command.guild_id) {
        // Send a message to the user, saying that a server administrator needs to accept the eula
        // }

        let command_id = command.data.id;
        let command_name = command.data.name.as_str();

        debug!(name = %command_name, id = %command_id, "Handling application command");

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
                    .utilities
                    .http_client
                    .interaction(self.utilities.application_id)
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
                self.admin_command_handler.on_command_called(command).await;
            }
            _ => debug!(command_name = %command_name, "Unhandled application command"),
        }

        Ok(())
    }

    pub async fn on_message_component_event(
        &self,
        message: &MessageComponentInteraction,
    ) -> Result<(), Box<dyn Error>> {
        debug!(message = %format!("{:?}", message), "TODO: handle message component interaction");

        // TODO: respond to message component interactions

        Ok(())
    }
}

impl ApplicationCommandUtilities {
    pub async fn new(db: Arc<Box<DatabaseConnection>>) -> Result<Self, Box<dyn Error>> {
        let http_client = DiscordHttpClient::new(crate::CONFIG.token.clone());
        let application_id = {
            let response = http_client.current_user_application().exec().await?;
            response.model().await?.id
        };

        Ok(Self::new_with_application_id(db, application_id))
    }

    pub fn new_with_application_id(
        db: Arc<Box<DatabaseConnection>>,
        application_id: Id<ApplicationMarker>,
    ) -> Self {
        Self {
            db,
            http_client: DiscordHttpClient::new(crate::CONFIG.token.clone()),
            application_id,
        }
    }

    pub async fn register_all_application_commands(&self) -> Result<(), Box<dyn Error>> {
        let debug_guild = crate::CONFIG.debug_guild_id.clone();

        let commands = vec![
            PingCommandHander::to_command(debug_guild),
            MatchmakingCommandHandler::to_command(debug_guild),
            AdminCommandHandler::to_command(debug_guild),
            EULACommandHandler::to_command(debug_guild),
        ];

        // TODO: In the future, only set as guild commands if we're not running in production mode or the debug_guild is not empty
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

    async fn send_message(
        &self,
        command: &Box<DiscordApplicationCommand>,
        message: &InteractionResponse,
    ) -> Result<(), Box<dyn Error>> {
        let _res = self
            .http_client
            .interaction(self.application_id)
            .interaction_callback(command.id, command.token.as_str(), message)
            .exec()
            .await?;

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
