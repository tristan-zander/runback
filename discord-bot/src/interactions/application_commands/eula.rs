use std::{error::Error, sync::Arc};

use chrono::Utc;
use entity::sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::CommandType,
        interaction::application_command::{
            ApplicationCommand as DiscordApplicationCommand, CommandOptionValue,
        },
    },
    channel::message::MessageFlags,
};
use twilight_util::builder::{
    command::{CommandBuilder, StringBuilder},
    CallbackDataBuilder,
};

use crate::RunbackError;

use super::{ApplicationCommand, ApplicationCommandUtilities};

// Consider getting this path from an environment variable
const EULA: &'static str = include_str!("../../../../EULA.md");

pub(super) struct EULACommandHandler {
    pub command_utils: Arc<ApplicationCommandUtilities>,
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
    pub fn new(command_utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self { command_utils }
    }

    pub async fn on_command_called(
        &self,
        command: &Box<DiscordApplicationCommand>,
    ) -> Result<(), RunbackError> {
        debug!(options = %format!("{:?}", command.data.options));

        let gid = if let Some(gid) = command.guild_id {
            gid
        } else {
            let message = InteractionResponse::ChannelMessageWithSource(
                CallbackDataBuilder::new()
                    .content("You cannot use this command in a DM.".into())
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            );
            return self.command_utils.send_message(command, &message).await;
        };

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
                        self.command_utils.send_message(command, &message).await?;

                        error!(
                            accepted = %accepted.clone(),
                            "Received unexpected value instead of accepting the EULA"
                        );

                        return Ok(());
                    }

                    let res = entity::matchmaking::Setting::find_by_id(gid.into())
                        .one(self.command_utils.db_ref())
                        .await?;
                    match res {
                        Some(existing_settings) => {
                            if existing_settings.has_accepted_eula.is_some() {
                                let message = InteractionResponse::ChannelMessageWithSource(
                                    CallbackDataBuilder::new()
                                        .content(
                                            "Looks like you've already accepted the EULA.".into(),
                                        )
                                        .flags(MessageFlags::EPHEMERAL)
                                        .build(),
                                );
                                self.command_utils.send_message(command, &message).await?;
                                return Ok(());
                            } else {
                                let mut active = existing_settings.into_active_model();
                                active.has_accepted_eula = Set(Some(Utc::now()));
                                active
                                    .update(self.command_utils.db_ref())
                                    .await?;
                            }
                        }
                        None => {
                            let settings = entity::matchmaking::settings::ActiveModel {
                                guild_id: Set(gid.into()),
                                has_accepted_eula: Set(Some(Utc::now())),
                                last_updated: Set(Utc::now()),
                                ..Default::default()
                            };

                            settings
                                .insert(self.command_utils.db_ref())
                                .await?;
                        }
                    };

                    let message = InteractionResponse::ChannelMessageWithSource(
                            CallbackDataBuilder::new()
                                .content("Okay, thanks for accepted the EULA. You may now use Runback's services.".into())
                                .flags(MessageFlags::EPHEMERAL)
                                .build(),
                        );
                    self.command_utils.send_message(command, &message).await?;
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

        self.command_utils.send_message(command, &message).await?;

        Ok(())
    }

    async fn send_message(
        &self,
        command: &Box<DiscordApplicationCommand>,
        message: &InteractionResponse,
    ) -> Result<(), Box<dyn Error>> {
        let _res = self
            .command_utils
            .http_client
            .interaction(self.command_utils.application_id)
            .interaction_callback(command.id, command.token.as_str(), message)
            .exec()
            .await?;

        Ok(())
    }
}
