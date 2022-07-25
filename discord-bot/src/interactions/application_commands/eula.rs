use std::sync::Arc;

use chrono::Utc;
use entity::sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use twilight_model::{
    application::{command::CommandType, interaction::application_command::CommandOptionValue},
    channel::message::MessageFlags,
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::command::{CommandBuilder, StringBuilder};
use twilight_util::builder::InteractionResponseDataBuilder as CallbackDataBuilder;

use super::{
    ApplicationCommandUtilities, CommandGroupDescriptor, InteractionData, InteractionHandler,
};

// Consider getting this path from an environment variable
const EULA: &'static str = include_str!("../../../../EULA.md");

pub struct EulaCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl InteractionHandler for EulaCommandHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        let builder = CommandBuilder::new(
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

        let command = builder.build();
        CommandGroupDescriptor {
            name: "EULA",
            description: "Read and accept the EULA",
            commands: Box::new([command]),
        }
    }

    async fn process_command(&self, data: Box<InteractionData>) -> anyhow::Result<()> {
        let command = &data.command;
        debug!(options = %format!("{:?}", command.data.options));

        let gid = if let Some(gid) = command.guild_id {
            gid
        } else {
            let message = InteractionResponse {
                data: Some(
                    CallbackDataBuilder::new()
                        .content("You cannot use this command in a DM.".into())
                        .flags(MessageFlags::EPHEMERAL)
                        .build(),
                ),
                kind: InteractionResponseType::ChannelMessageWithSource,
            };
            self.utils
                .send_message(command, &message)
                .await
                .map_err(|e| anyhow!("Could not send message: {}", e))?;

            return Ok(());
        };

        let options = &command.data.options;
        if options.len() > 0 && options[0].name.as_str() == "accept" {
            match &options[0].value {
                CommandOptionValue::String(accepted) => {
                    if accepted.as_str() != "accept" {
                        let message = InteractionResponse {
                            data: Some(
                            CallbackDataBuilder::new()
                                .content("You must accept the EULA to use Runback. Run \"/eula\" without any arguments to see the EULA.".into())
                                .flags(MessageFlags::EPHEMERAL)
                                .build()
                        ),
                            kind: InteractionResponseType::ChannelMessageWithSource,
                    };
                        self.utils
                            .send_message(command, &message)
                            .await
                            .map_err(|e| anyhow!("Could not send message: {}", e))?;

                        error!(
                            accepted = %accepted.clone(),
                            "Received unexpected value instead of accepting the EULA"
                        );

                        return Ok(());
                    }

                    let res = entity::matchmaking::Setting::find_by_id(gid.into())
                        .one(self.utils.db_ref())
                        .await?;
                    match res {
                        Some(existing_settings) => {
                            if existing_settings.has_accepted_eula.is_some() {
                                let message = InteractionResponse {
                                    data: Some(
                                        CallbackDataBuilder::new()
                                            .content(
                                                "Looks like you've already accepted the EULA."
                                                    .into(),
                                            )
                                            .flags(MessageFlags::EPHEMERAL)
                                            .build(),
                                    ),
                                    kind: InteractionResponseType::ChannelMessageWithSource,
                                };
                                self.utils
                                    .send_message(command, &message)
                                    .await
                                    .map_err(|e| anyhow!("Could not send message: {}", e))?;
                                return Ok(());
                            } else {
                                let mut active = existing_settings.into_active_model();
                                active.has_accepted_eula = Set(Some(Utc::now()));
                                let db_ref = self.utils.db_ref();
                                async move { active.update(db_ref).await }.await?;
                            }
                        }
                        None => {
                            let settings = entity::matchmaking::settings::ActiveModel {
                                guild_id: Set(gid.into()),
                                has_accepted_eula: Set(Some(Utc::now())),
                                last_updated: Set(Utc::now()),
                                ..Default::default()
                            };

                            settings.insert(self.utils.db_ref()).await?;
                        }
                    };

                    let message = InteractionResponse { kind: InteractionResponseType::ChannelMessageWithSource, data: Some(
                            CallbackDataBuilder::new()
                                .content("Okay, thanks for accepted the EULA. You may now use Runback's services.".into())
                                .flags(MessageFlags::EPHEMERAL)
                                .build(),
                        )};
                    self.utils
                        .send_message(command, &message)
                        .await
                        .map_err(|e| anyhow!("Could not send message: {}", e))?;
                }
                _ => {}
            }
        }

        let message = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
                CallbackDataBuilder::new()
                    .content(EULA.into())
                    .flags(MessageFlags::EPHEMERAL)
                    .build(),
            ),
        };

        self.utils
            .send_message(command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?;

        Ok(())
    }

    async fn process_autocomplete(&self, _data: Box<InteractionData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_modal(&self, _data: Box<InteractionData>) -> anyhow::Result<()> {
        unreachable!()
    }

    async fn process_component(&self, _data: Box<InteractionData>) -> anyhow::Result<()> {
        unreachable!()
    }
}

impl EulaCommandHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self { utils }
    }
}
