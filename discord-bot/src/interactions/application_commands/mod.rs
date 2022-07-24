pub mod admin;
pub mod eula;
pub mod lfg;
pub mod matchmaking;

use std::{pin::Pin, sync::Arc};

use entity::sea_orm::{prelude::Uuid, DatabaseConnection};
use futures::Future;
use twilight_cache_inmemory::InMemoryCache;
use twilight_http::Client as DiscordHttpClient;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        interaction::ApplicationCommand as DiscordApplicationCommand,
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

#[macro_export]
macro_rules! handler {
    ($func:expr) => {
        |a, d| Box::pin($func(a, d))
    };
}

pub type HandlerType = fn(
    Arc<ApplicationCommandUtilities>,
    Box<InteractionData>,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>>;

/// Contains any helper functions to help make writing application command handlers easier
/// Make sure this is thread safe
pub struct ApplicationCommandUtilities {
    pub http_client: DiscordHttpClient,
    pub application_id: Id<ApplicationMarker>,
    pub db: Arc<Box<DatabaseConnection>>,
    pub cache: Arc<InMemoryCache>,
    pub standby: Arc<Standby>,
}

impl ApplicationCommandUtilities {
    pub async fn new(
        db: Arc<Box<DatabaseConnection>>,
        cache: Arc<InMemoryCache>,
        standby: Arc<Standby>,
    ) -> anyhow::Result<Self> {
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

/// Describes a group of commands. This is mainly used
/// for structural purposes, and for the `/help` command
#[derive(Debug, Clone)]
pub struct CommandGroupDescriptor {
    /// The name of the command group
    pub name: &'static str,
    /// The description of the command group
    pub description: &'static str,
    /// The commands that are releated to this group
    pub commands: Box<[CommandDescriptor]>,
}

/// Describes a single command. This is used for the `/help`
/// command and to register the command with Discord
#[derive(Debug, Clone)]
pub struct CommandDescriptor {
    pub command: Command,
    // pub guild_id: Option<Id<GuildMarker>>
    pub handler: Option<HandlerType>,
}

pub trait ApplicationCommandHandler {
    fn register(&self) -> CommandGroupDescriptor;

    // async fn execute(&self, data: &InteractionData) -> anyhow::Result<()>;
}

pub struct InteractionData {
    pub command: ApplicationCommand,
    pub id: Uuid,
    // pub cancellation_token
}

pub struct PingCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

impl ApplicationCommandHandler for PingCommandHandler {
    fn register(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
            "ping".to_string(),
            "Responds with pong".into(),
            CommandType::ChatInput,
        )
        .option(StringBuilder::new(
            "text".into(),
            "Send this text alongside the response".into(),
        ));

        let command = builder.build();
        debug!(command = %format!("{:?}", command), "Created command");
        return CommandGroupDescriptor {
            name: "ping",
            description: "Commands that relate to response time",
            commands: Box::new([CommandDescriptor {
                handler: Some(handler!(PingCommandHandler::execute)),
                command,
            }]),
        };
    }
}

impl PingCommandHandler {
    async fn execute(
        utils: Arc<ApplicationCommandUtilities>,
        data: Box<InteractionData>,
    ) -> anyhow::Result<()> {
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

        let _res = utils
            .http_client
            .interaction(utils.application_id)
            .create_response(data.command.id, data.command.token.as_str(), &message)
            .exec()
            .await?;

        debug!(message = %format!("{:?}", message), "Reponded to command \"Pong\"");

        Ok(())
    }
}
