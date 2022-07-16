pub mod admin;
pub mod eula;
pub mod lfg;
pub mod matchmaking;

use std::sync::Arc;

use entity::sea_orm::DatabaseConnection;
use twilight_cache_inmemory::InMemoryCache;
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
    id::{marker::ApplicationMarker, Id},
};
use twilight_standby::Standby;
use twilight_util::builder::command::{CommandBuilder, StringBuilder};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder};
use twilight_util::builder::InteractionResponseDataBuilder;

use twilight_model::application::interaction::ApplicationCommand;

use crate::error::RunbackError;

/// Contains any helper functions to help make writing application command handlers easier
/// Make sure this is thread safe
pub struct ApplicationCommandUtilities {
    pub http_client: DiscordHttpClient,
    pub application_id: Id<ApplicationMarker>,
    pub db: Arc<Box<DatabaseConnection>>,
    pub cache: Arc<InMemoryCache>,
    pub standby: Arc<Standby>,
}

pub struct ApplicationCommandHandlers {
    pub utils: Arc<ApplicationCommandUtilities>,
}

impl ApplicationCommandHandlers {
    pub async fn new(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> Result<Self, RunbackError> {
        let utilities = Arc::new(ApplicationCommandUtilities::new(db, cache, standby).await?);
        Ok(Self {
            utils: utilities.clone(),
        })
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
                // self.admin_command_handler
                //     .on_message_component_event(id_parts, message)
                //     .await?
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
                // self.admin_command_handler
                //     .on_modal_submit(id_parts, modal)
                //     .await?
            }
            _ => {
                warn!(custom_id = %custom_id, "Unknown message component event")
            }
        };

        Ok(())
    }
}

impl ApplicationCommandUtilities {
    pub async fn new(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> Result<Self, RunbackError> {
        let http_client = DiscordHttpClient::new(crate::CONFIG.token.clone());
        let application_id = {
            let response = http_client.current_user_application().exec().await?;
            response.model().await?.id
        };

        Ok(Self::new_with_application_id(
            db,
            application_id,
            cache,
            standby,
        ))
    }

    pub fn new_with_application_id(
        db: Arc<Box<DatabaseConnection>>,
        application_id: Id<ApplicationMarker>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> Self {
        Self {
            db,
            http_client: DiscordHttpClient::new(crate::CONFIG.token.clone()),
            application_id,
            cache,
            standby,
        }
    }

    pub fn db_ref(&self) -> &DatabaseConnection {
        (*self.db).as_ref()
    }

    async fn send_message(
        &self,
        command: &DiscordApplicationCommand,
        message: &InteractionResponse,
    ) -> Result<(), RunbackError> {
        let res = self
            .http_client
            .interaction(self.application_id)
            .create_response(command.id, command.token.as_str(), message)
            .exec()
            .await?;

        debug!("Send Message response: {:#?}", res);

        Ok(())
    }
}

#[non_exhaustive]
pub enum CommandHandlerType {
    TopLevel(Command),
    SubCommand,
}

#[async_trait]
pub trait ApplicationCommandHandler {
    fn name(&self) -> String;

    fn register(&self) -> CommandHandlerType;

    async fn execute(&self, data: &InteractionData) -> anyhow::Result<()>;
}

pub struct InteractionData<'a> {
    pub command: &'a ApplicationCommand,
}

pub struct PingCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl ApplicationCommandHandler for PingCommandHandler {
    fn name(&self) -> String {
        "ping".into()
    }

    fn register(&self) -> CommandHandlerType {
        let mut builder = CommandBuilder::new(
            self.name(),
            "Responds with pong".into(),
            CommandType::ChatInput,
        )
        .option(StringBuilder::new(
            "text".into(),
            "Send this text alongside the response".into(),
        ));

        let comm = builder.build();
        debug!(comm = %format!("{:?}", comm), "Created command");
        return CommandHandlerType::TopLevel(comm);
    }

    async fn execute(&self, data: &InteractionData) -> anyhow::Result<()> {
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
            .create_response(data.command.id, data.command.token.as_str(), &message)
            .exec()
            .await?;

        debug!(message = %format!("{:?}", message), "Reponded to command \"Pong\"");

        Ok(())
    }
}
