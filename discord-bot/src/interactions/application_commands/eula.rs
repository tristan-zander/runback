use std::error::Error;

use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::CommandType,
        interaction::application_command::{
            ApplicationCommand as DiscordApplicationCommand, CommandOptionValue,
        },
    },
    channel::message::MessageFlags,
    id::{marker::ApplicationMarker, Id},
};
use twilight_util::builder::{
    command::{CommandBuilder, StringBuilder},
    CallbackDataBuilder,
};

use super::ApplicationCommand;

// Consider getting this path from an environment variable
const EULA: &'static str = include_str!("../../../../EULA.txt");

#[derive(Debug)]
pub(super) struct EULACommandHandler {
    pub http_client: twilight_http::Client,
    pub application_id: Id<ApplicationMarker>,
}

impl ApplicationCommand for EULACommandHandler {
    fn to_command(
        debug_guild: Option<twilight_model::id::Id<twilight_model::id::marker::GuildMarker>>,
    ) -> twilight_model::application::command::Command {
        let mut builder = CommandBuilder::new(
            "eula".into(),
            "Show the EULA".into(),
            CommandType::ChatInput,
        )
        .option(
            StringBuilder::new("accept".into(), "Accept the EULA (admin only)".into()).choices(
                vec![(
                    "I have read the EULA and agree to its terms.".into(),
                    "accept".into(),
                )],
            ),
        );

        if let Some(id) = debug_guild {
            builder = builder.guild_id(id);
        }

        let comm = builder.build();
        debug!(comm = %format!("{:?}", comm), "Created command");
        return comm;
    }
}

impl EULACommandHandler {
    #[tracing::instrument]
    pub async fn on_command_called(
        &self,
        command: &Box<twilight_model::application::interaction::ApplicationCommand>,
    ) -> Result<(), Box<dyn Error>> {
        debug!(options = %format!("{:?}", command.data.options));

        let options = &command.data.options;
        if options.len() > 0 && options[0].name.as_str() == "accept" {
            match &options[0].value {
                CommandOptionValue::String(accepted) => {
                    if accepted.as_str() != "accept" {
                        let message = InteractionResponse::ChannelMessageWithSource(
                            CallbackDataBuilder::new()
                                .content("You must accept the EULA to use Runback. Run \"/eula\" without any arguments to see the EULA.".into())
                                .flags(MessageFlags::EPHEMERAL)
                                .build(),
                        );
                        self.send_message(command, &message).await?;

                        error!(
                            accepted = %accepted.clone(),
                            "Received unexpected value instead of accepting the EULA"
                        );

                        return Ok(());
                    }
                    let message = InteractionResponse::ChannelMessageWithSource(
                            CallbackDataBuilder::new()
                                .content("Okay, thanks for accepted the EULA. You may now use Runback's services.".into())
                                .flags(MessageFlags::EPHEMERAL)
                                .build(),
                        );
                    self.send_message(command, &message).await?;
                }
                _ => {}
            }
        }

        let message = InteractionResponse::ChannelMessageWithSource(
            CallbackDataBuilder::new()
                .content(EULA.into())
                .flags(MessageFlags::EPHEMERAL)
                .build(),
        );

        self.send_message(command, &message).await?;

        Ok(())
    }

    #[tracing::instrument]
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
