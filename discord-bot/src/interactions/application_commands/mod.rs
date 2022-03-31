mod admin;
mod eula;
mod matchmaking;

use std::sync::Arc;

use entity::sea_orm::DatabaseConnection;
use twilight_http::Client as DiscordHttpClient;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::{
            modal::ModalSubmitInteraction, ApplicationCommand as DiscordApplicationCommand,
            MessageComponentInteraction,
        },
    },
    channel::message::MessageFlags,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ApplicationMarker, GuildMarker},
        Id,
    },
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::{
    error::RunbackError, interactions::application_commands::matchmaking::MatchmakingCommandHandler,
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
    pub utils: Arc<ApplicationCommandUtilities>,
    eula_command_handler: EULACommandHandler,
    admin_command_handler: AdminCommandHandler,
}

impl ApplicationCommandHandlers {
    pub async fn new(db: Arc<Box<DatabaseConnection>>) -> Result<Self, RunbackError> {
        let utilities = Arc::new(ApplicationCommandUtilities::new(db).await?);
        Ok(Self {
            utils: utilities.clone(),
            eula_command_handler: EULACommandHandler::new(utilities.clone()),
            admin_command_handler: AdminCommandHandler::new(utilities.clone()),
        })
    }

    pub async fn on_command_receive(
        &self,
        command: &DiscordApplicationCommand,
    ) -> Result<(), RunbackError> {
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

                let message = InteractionResponse {
                    data: Some(
                        InteractionResponseDataBuilder::new()
                            .embeds(vec![
                                EmbedBuilder::new()
                                    .color(0x55_4e_2b)
                                    .description("Runback Matchmaking Bot")
                                    .field(EmbedFieldBuilder::new("Ping?", "Pong!").build())
                                    .build(), // .map_err(|e| RunbackError {
                                              //     message: "Failed to build callback data".to_owned(),
                                              //     inner: Some(e.into()),
                                              // })?
                            ])
                            .flags(MessageFlags::EPHEMERAL)
                            .build(),
                    ),
                    kind: InteractionResponseType::ChannelMessageWithSource,
                };

                let _res = self
                    .utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_response(command.id, command.token.as_str(), &message)
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
                self.admin_command_handler
                    .on_command_called(command)
                    .await?;
            }
            _ => warn!(command_name = %command_name, "Unhandled application command"),
        }

        Ok(())
    }

    pub async fn on_message_component_event(
        &self,
        message: &MessageComponentInteraction,
    ) -> Result<(), RunbackError> {
        debug!(message = %format!("{:?}", message), "TODO: handle message component interaction");

        let custom_id = &message.data.custom_id;
        let _component_type = message.data.component_type;

        let id_parts = custom_id.split(':').collect::<Vec<_>>();
        let namespace: &str = id_parts[0];

        let _res = match namespace {
            "admin" => {
                self.admin_command_handler
                    .on_message_component_event(id_parts, message)
                    .await?
            }
            _ => {
                warn!(custom_id = %custom_id, "Unknown message component event")
            }
        };

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn on_modal_submit(
        &self,
        modal: &ModalSubmitInteraction,
    ) -> Result<(), RunbackError> {
        debug!(modal = %format!("{:?}", modal), "TODO: handle message component interaction");

        let custom_id = modal.data.custom_id.as_str();

        let id_parts = custom_id.split(':').collect::<Vec<_>>();
        let namespace: &str = id_parts[0];

        let _res = match namespace {
            "admin" => {
                self.admin_command_handler
                    .on_modal_submit(id_parts, modal)
                    .await?
            }
            _ => {
                warn!(custom_id = %custom_id, "Unknown message component event")
            }
        };

        Ok(())
    }
}

impl ApplicationCommandUtilities {
    pub async fn new(db: Arc<Box<DatabaseConnection>>) -> Result<Self, RunbackError> {
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

    pub fn db_ref(&self) -> &DatabaseConnection {
        (*self.db).as_ref()
    }

    pub async fn register_all_application_commands(&self) -> Result<(), RunbackError> {
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
        command: &DiscordApplicationCommand,
        message: &InteractionResponse,
    ) -> Result<(), RunbackError> {
        let _res = self
            .http_client
            .interaction(self.application_id)
            .create_response(command.id, command.token.as_str(), message)
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
