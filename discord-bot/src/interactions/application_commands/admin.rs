use std::{error::Error, sync::Arc};

use chrono::Utc;
use entity::sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel};
use twilight_embed_builder::EmbedBuilder;
use twilight_model::{
    application::{
        callback::{CallbackData, InteractionResponse},
        command::{Command, CommandType},
        component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand as DiscordApplicationCommand, MessageComponentInteraction,
        },
    },
    channel::{message::MessageFlags, GuildChannel, TextChannel},
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder, SubCommandGroupBuilder},
    CallbackDataBuilder,
};

use crate::RunbackError;

use super::{ApplicationCommand, ApplicationCommandUtilities};

#[derive(Debug)]
struct AdminCommandHandlerError {
    message: &'static str,
}

impl std::fmt::Display for AdminCommandHandlerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for AdminCommandHandlerError {}

pub(super) struct AdminCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

impl ApplicationCommand for AdminCommandHandler {
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command {
        let mut builder = CommandBuilder::new(
            "admin".into(),
            "Admin configuration and management settings".into(),
            CommandType::ChatInput,
        )
        .option(SubCommandBuilder::new(
            "matchmaking-settings".into(),
            "Shows the matchmaking settings panel".into(),
        ));

        if let Some(id) = debug_guild {
            builder = builder.guild_id(id);
        }

        let comm = builder.build();
        debug!(command = %format!("{:?}", comm), "Created command!");
        return comm;
    }
}

impl AdminCommandHandler {
    pub fn new(command_utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self {
            utils: command_utils,
        }
    }

    pub async fn on_command_called(&self, command: &DiscordApplicationCommand) {
        let options = &command.data.options;

        // There should only be one subcommand option, but map through them anyways
        for option in options {
            match option.name.as_str() {
                "matchmaking-settings" => {
                    self.send_matchamking_settings(command).await.unwrap();
                    return;
                }
                _ => {
                    debug!(name = %option.name.as_str(), "Unknown admin subcommand option")
                }
            }
        }
    }

    pub async fn on_message_component_event(
        &self,
        id_parts: Vec<&str>,
        component: &MessageComponentInteraction,
    ) -> Result<(), RunbackError> {
        let sub_group = *id_parts.get(1).unwrap();
        let action_id = *id_parts.get(2).unwrap();

        match sub_group {
            "mm" => {
                // Matchmaking settings handler
                match action_id {
                    "channel" => {
                        self.set_matchmaking_channel(component).await?;
                    }
                    _ => {
                        warn!(action = %action_id, group = %sub_group, "Unknown admin custom action received")
                    }
                }
            }
            _ => {
                warn!(sub_group = %sub_group, custom_id = %&component.data.custom_id, "Unknown admin component received")
            }
        }

        Ok(())
    }

    async fn set_matchmaking_channel(
        &self,
        component: &MessageComponentInteraction,
    ) -> Result<(), RunbackError> {
        let channel_id: Id<ChannelMarker> = Id::new(
            component
                .data
                .values
                .get(0)
                .ok_or("No component values provided.")?
                .parse::<u64>()
                .map_err(|e| -> RunbackError {RunbackError {
                    message: "Unable to parse channel_id. Data is invalid".to_owned(),
                    inner: Some(e.into()),
                }})?,
        );

        let guild_id = component
            .guild_id
            .ok_or("You cannot use Runback in a DM.")?;

        let setting = entity::matchmaking::Setting::find_by_id(guild_id.into())
            .one(self.utils.db_ref())
            .await?;

        let setting = if setting.is_some() {
            let mut setting = unsafe { setting.unwrap_unchecked() }.into_active_model();
            setting.channel_id = entity::sea_orm::Set(Some(channel_id.into()));
            setting.update(self.utils.db_ref()).await?
        } else {
            let setting = entity::matchmaking::settings::Model {
                guild_id: guild_id.into(),
                last_updated: Utc::now(),
                channel_id: Some(channel_id.into()),
                has_accepted_eula: None,
                threads_are_private: false,
            }
            .into_active_model();
            setting
                .into_active_model()
                .insert(self.utils.db_ref())
                .await?
        };

        // TODO: Produce a Kafka message, saying that this guild's settings have been updated
        let message = InteractionResponse::UpdateMessage(
            CallbackDataBuilder::new()
                .flags(MessageFlags::EPHEMERAL)
                .content("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect.".into())
                .build()
        );

        let _res = self
            .utils
            .http_client
            .interaction(self.utils.application_id)
            .interaction_callback(component.id, component.token.as_str(), &message)
            .exec()
            .await?;

        Ok(())
    }

    pub async fn send_matchamking_settings(
        &self,
        command: &DiscordApplicationCommand,
    ) -> Result<(), Box<dyn Error>> {
        // VERIFY: Is it possible that we can send the information of other guilds here?
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err(AdminCommandHandlerError {
                    message: "Can't find a guild id for this command.",
                }
                .into());
            }
        };

        let channels = self
            .utils
            .http_client
            .guild_channels(guild_id)
            .exec()
            .await?
            .models()
            .await?;

        let text_channels = channels
            .iter()
            .filter_map(|c| {
                let val = match c {
                    GuildChannel::Text(t) => Some(t),
                    _ => None,
                };
                val
            })
            .collect::<Vec<&TextChannel>>();

        debug!(channels = %format!("{:?}", text_channels), "Collected text channels");

        let message = InteractionResponse::ChannelMessageWithSource(
            CallbackDataBuilder::new()
                .flags(MessageFlags::EPHEMERAL)
                .components(vec![Component::ActionRow(ActionRow {
                    components: vec![Component::SelectMenu(SelectMenu {
                        custom_id: "admin:mm:channel".into(),
                        disabled: false,
                        max_values: Some(1),
                        min_values: Some(1),
                        options: text_channels
                            .iter()
                            .map(|chan| SelectMenuOption {
                                default: false,
                                description: None,
                                emoji: None,
                                label: format!("#{}", chan.name),
                                value: chan.id.to_string(),
                            })
                            .collect::<Vec<SelectMenuOption>>(),
                        placeholder: Some("Select the default matchmaking channel".into()),
                    })],
                })])
                .build(),
        );

        Ok(self.utils.send_message(command, &message).await?)
    }

    pub async fn handle_matchmaking_settings_changed() {}
}
