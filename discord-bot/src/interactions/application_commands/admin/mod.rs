use std::{error::Error, sync::Arc};

use chrono::Utc;
use entity::{
    sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter},
    IdWrapper,
};
use twilight_model::{
    application::{
        command::{Command, CommandType},
        component::{
            button::ButtonStyle, select_menu::SelectMenuOption, ActionRow, Button, Component,
            SelectMenu,
        },
        interaction::{
            ApplicationCommand as DiscordApplicationCommand, MessageComponentInteraction,
        },
    },
    channel::{message::MessageFlags, Channel, ChannelType},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder},
    embed::EmbedBuilder,
    InteractionResponseDataBuilder as CallbackDataBuilder,
};

use crate::RunbackError;

use super::{ApplicationCommand, ApplicationCommandUtilities};

mod panel_model;

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
            "mm-panels".into(),
            "Add, edit, and remove matchmaking panels in your guild".into(),
        ))
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

    pub async fn on_command_called(
        &self,
        command: &DiscordApplicationCommand,
    ) -> Result<(), RunbackError> {
        let options = &command.data.options;

        // There should only be one subcommand option, but map through them anyways
        for option in options {
            match option.name.as_str() {
                "matchmaking-settings" => {
                    self.send_matchamking_settings(command).await?;
                }
                "mm-panels" => {
                    self.on_mm_panels_command_received(command).await?;
                }
                _ => {
                    debug!(name = %option.name.as_str(), "Unknown admin subcommand option")
                }
            }
        }

        Ok(())
    }

    pub async fn on_message_component_event(
        &self,
        id_parts: Vec<&str>,
        component: &MessageComponentInteraction,
    ) -> Result<(), RunbackError> {
        let sub_group = *id_parts
            .get(1)
            .ok_or("Could not get message component sub_group")?;
        let action_id = *id_parts
            .get(2)
            .ok_or("Could not get message component action_id")?;

        match sub_group {
            "mm" => {
                // Matchmaking settings handler
                match action_id {
                    "channel" => {
                        self.set_matchmaking_channel(component).await?;
                    }
                    "panels" => {
                        let component_id = *id_parts
                            .get(3)
                            .ok_or("Could not get message component action_id")?;
                        let args = &id_parts[3..];
                        self.on_mm_panel_component_changed(component, component_id, args)
                            .await?;
                    }
                    _ => {
                        warn!(action = %action_id, group = %sub_group, parts = %format!("{:?}", id_parts), "Unknown admin custom action received")
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
                .map_err(|e| -> RunbackError {
                    RunbackError {
                        message: "Unable to parse channel_id. Data is invalid".to_owned(),
                        inner: Some(e.into()),
                    }
                })?,
        );

        let guild_id = component
            .guild_id
            .ok_or("You cannot use Runback in a DM.")?;

        let setting = entity::matchmaking::Setting::find_by_id(guild_id.into())
            .one(self.utils.db_ref())
            .await?;

        let _setting = if setting.is_some() {
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
        let message = InteractionResponse { kind: InteractionResponseType::UpdateMessage, data: Some(
            CallbackDataBuilder::new()
                .flags(MessageFlags::EPHEMERAL)
                .content("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect.".into())
                .build()
        )};

        let _res = self
            .utils
            .http_client
            .interaction(self.utils.application_id)
            .update_response(component.token.as_str())
            .content(Some("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect"))?
            // .map_err(|e| RunbackError { message: "Could not set content for response message during set_matchmaking_channel()".to_owned(), inner: Some(Box::new(e)) })?
            // .(component.id, component.token.as_str(), &message)
            .exec()
            .await?;

        Ok(())
    }

    /// Called whenever `/admin mm-panels` is called by an admin user.
    #[tracing::instrument(skip_all)]
    async fn on_mm_panels_command_received(
        &self,
        command: &DiscordApplicationCommand,
    ) -> Result<(), RunbackError> {
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err("Can't find a guild id for this command.".into());
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
            .into_iter()
            .filter_map(|c| {
                let val = if let ChannelType::GuildText = c.kind {
                    Some(c)
                } else {
                    None
                };
                val
            })
            .collect::<Vec<Channel>>();

        let embed = EmbedBuilder::new()
            .title("Admin Panel")
            .description("Please select a panel.");
        let mut callback_data = CallbackDataBuilder::new().flags(MessageFlags::EPHEMERAL);

        let panels = entity::matchmaking::Panel::find()
            .filter(
                entity::matchmaking::panel::Column::GuildId
                    .eq(Into::<IdWrapper<GuildMarker>>::into(guild_id)),
            )
            .all(self.utils.db_ref())
            .await?;

        let select_menu_options: Vec<_> = panels
            .iter()
            .filter_map(|p| {
                let text_channel = text_channels
                    .iter()
                    .filter(|t| p.channel_id == t.id.into())
                    .collect::<Vec<_>>();

                if text_channel.len() != 1 {
                    warn!(id = %p.channel_id, channels = %format!("{:?}", text_channel), "Found multiple or no text channels by single id");
                    return None;
                }

                let text_channel = text_channel[0];

                let name = if let Some(n) = &text_channel.name {
                    n.as_str()
                } else {
                    "Unknown Channel Name"
                };

                Some(SelectMenuOption {
                    default: false,
                    description: p.game.to_owned(),
                    emoji: None,
                    label: format!("#{}", name),
                    value: text_channel.id.to_string(),
                })
            })
            .collect();

        let mut components = Vec::new();

        if select_menu_options.len() > 0 {
            let select_menu_row = Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "admin:mm:panels:select_existing".into(),
                    disabled: false,
                    max_values: Some(1),
                    min_values: Some(1),
                    options: select_menu_options,
                    placeholder: Some("Select a panel".into()),
                })],
            });
            components.push(select_menu_row);
        }

        // When this button is called, update the embed and components of the original message
        components.push(Component::ActionRow(ActionRow {
            components: vec![Component::Button(Button {
                custom_id: Some("admin:mm:panels:add_new".into()),
                disabled: false,
                emoji: None,
                label: Some("New Panel".into()),
                style: ButtonStyle::Primary,
                url: None,
            })],
        }));

        callback_data = callback_data
            .embeds(vec![embed.build()])
            .components(components);
        let message = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(callback_data.build()),
        };

        self.utils.send_message(command, &message).await?;

        Ok(())
    }

    /// Called whenever an admin interacts with the mm panel.
    #[tracing::instrument(skip_all)]
    async fn on_mm_panel_component_changed(
        &self,
        component: &MessageComponentInteraction,
        component_id: &str,
        args: &[&str],
    ) -> Result<(), RunbackError> {
        match component_id {
            "add_new" => {
                // If the user is sending all the component data
                if args.len() > 0 && args[0] == "create" {
                    // Create the new panel
                    self.create_new_mm_panel(component).await?;
                    // Send the new panel with the interaction data
                }

                // Otherwise, update the message with all of the fields for the new panel
            }
            _ => {
                warn!(component_id, "Unknown component_id found");
            }
        }

        Ok(())
    }

    async fn create_new_mm_panel(
        &self,
        component: &MessageComponentInteraction,
    ) -> Result<(), RunbackError> {
        Ok(())
    }

    async fn send_matchamking_settings(
        &self,
        command: &DiscordApplicationCommand,
    ) -> Result<(), RunbackError> {
        // VERIFY: Is it possible that we can send the information of other guilds here?
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err("Can't find a guild id for this command.".into());
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
                let val = match c.kind {
                    ChannelType::GuildText => Some(c),
                    _ => None,
                };
                val
            })
            .collect::<Vec<&Channel>>();

        debug!(channels = %format!("{:?}", text_channels), "Collected text channels");

        let message = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(
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
                                    label: format!(
                                        "#{}",
                                        chan.name
                                            .as_ref()
                                            .expect("Guild text channel did not have a name")
                                            .as_str()
                                    ),
                                    value: chan.id.to_string(),
                                })
                                .collect::<Vec<SelectMenuOption>>(),
                            placeholder: Some("Select the default matchmaking channel".into()),
                        })],
                    })])
                    .build(),
            ),
        };

        Ok(self.utils.send_message(command, &message).await?)
    }

    // pub async fn handle_matchmaking_settings_changed() {}
}
