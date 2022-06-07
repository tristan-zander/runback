use std::{collections::HashMap, error::Error, sync::Arc};

use chrono::Utc;
use entity::sea_orm::{prelude::Uuid, ActiveModelTrait, EntityTrait, IntoActiveModel, Set};
use futures::StreamExt;
use twilight_gateway::Event;
use twilight_model::{
    application::{
        command::{Command, CommandType},
        component::{
            button::ButtonStyle, select_menu::SelectMenuOption, text_input::TextInputStyle,
            ActionRow, Button, Component, SelectMenu, TextInput,
        },
        interaction::{
            modal::{ModalInteractionData, ModalSubmitInteraction},
            ApplicationCommand as DiscordApplicationCommand, MessageComponentInteraction,
        },
    },
    channel::{message::MessageFlags, Channel, ChannelType},
    gateway::payload::incoming::InteractionCreate,
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    },
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder},
    embed::EmbedBuilder,
    InteractionResponseDataBuilder,
};

use crate::{
    interactions::{
        application_commands::{CommandHandlerType, InteractionData},
        panels::admin_lobby::{AdminLobbiesPanel, AdminLobbiesSinglePanel, MatchmakingPanel},
    },
    RunbackError,
};

use crate::interactions::application_commands::{
    ApplicationCommandHandler, ApplicationCommandUtilities,
};

use super::{
    mm_panels_handler::MatchmakingPanelsHandler, mm_settings_handler::MatchmakingSettingsHandler,
};

pub struct AdminCommandHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
    sub_commands: HashMap<String, Box<dyn ApplicationCommandHandler + Send + Sync + 'static>>,
}

#[async_trait]
impl ApplicationCommandHandler for AdminCommandHandler {
    fn name(&self) -> String {
        todo!()
    }

    fn register(&self) -> CommandHandlerType {
        let builder = CommandBuilder::new(
            "admin".into(),
            "Admin configuration and management settings".into(),
            CommandType::ChatInput,
        )
        .option(SubCommandBuilder::new(
            "matchmaking-panels".into(),
            "Add, edit, and remove matchmaking panels in your guild".into(),
        ))
        .option(SubCommandBuilder::new(
            "matchmaking-settings".into(),
            "Shows the matchmaking settings panel".into(),
        ));

        let comm = builder.build();
        return CommandHandlerType::TopLevel(comm);
    }

    async fn execute(&self, data: &InteractionData) -> anyhow::Result<()> {
        let options = &data.command.data.options;

        if options.len() != 1 {
            return Err(anyhow!("Expected extra options when calling the top-level admin command handler. Number of arguments found: {}", options.len()));
        }

        let option = &options[0];

        match self.sub_commands.get(&option.name) {
            Some(ref c) => {
                c.execute(data).await?;
            }
            _ => {
                return Err(anyhow!(
                    "No subcommand found with the name {}",
                    &option.name
                ))
            }
        }

        // There should only be one subcommand option, but map through them anyways
        // for option in options {
        //     match option.name.as_str() {
        //         "matchmaking-settings" => {
        //             self.send_matchamking_settings(data.command)
        //                 .await
        //                 .map_err(|e| anyhow!("Error with mm settings: {}", e))?;
        //         }
        //         "matchmaking-panels" => {
        //             self.on_mm_panels_command_received(data.command)
        //                 .await
        //                 .map_err(|e| anyhow!("Error with mm panel: {}", e))?;
        //         }
        //         _ => {
        //             debug!(name = %option.name.as_str(), "Unknown admin subcommand option");
        //             return Err(anyhow!("Unknown admin subcommand option"));
        //         }
        //     }
        // }

        Ok(())
    }
}

impl AdminCommandHandler {
    pub fn new(command_utils: Arc<ApplicationCommandUtilities>) -> Self {
        let mut admin_handler = Self {
            utils: command_utils,
            sub_commands: HashMap::new(),
        };

        let sub_handlers: Vec<Box<dyn ApplicationCommandHandler + Send + Sync + 'static>> = vec![
            Box::new(MatchmakingSettingsHandler {
                utils: admin_handler.utils.clone(),
            }),
            Box::new(MatchmakingPanelsHandler {
                utils: admin_handler.utils.clone(),
            }),
        ];

        for handler in sub_handlers {
            if let CommandHandlerType::SubCommand = handler.register() {
                let name = handler.name();
                admin_handler.sub_commands.insert(name.clone(), handler);
                debug!(name = %name, "Registered admin sub-command handler");
            }
        }

        admin_handler
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

    #[tracing::instrument(skip_all)]
    pub async fn on_modal_submit(
        &self,
        id_parts: Vec<&str>,
        modal: &ModalSubmitInteraction,
    ) -> Result<(), RunbackError> {
        let sub_group = *id_parts
            .get(1)
            .ok_or("Could not get message component sub_group")?;
        let action_id = *id_parts
            .get(2)
            .ok_or("Could not get message component action_id")?;

        match sub_group {
            "mm" => match action_id {
                "panel" => {
                    let modal_kind = *id_parts.get(3).ok_or("Could not get admin mm modal_kind")?;
                    let args = &id_parts[3..];
                    self.on_mm_panel_modal_submit(modal, modal_kind, args)
                        .await?;
                }
                _ => {
                    warn!(action = %action_id, group = %sub_group, parts = %format!("{:?}", id_parts), "Unknown matchmaking panel modal received")
                }
            },
            _ => {
                warn!(sub_group = %sub_group, custom_id = %&modal.data.custom_id, "Unknown admin modal received")
            }
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    async fn on_mm_panel_modal_submit(
        &self,
        modal: &ModalSubmitInteraction,
        modal_kind: &str,
        args: &[&str],
    ) -> Result<(), RunbackError> {
        match modal_kind {
            "new" => {
                let game_raw = modal.data.components[0].components[0].value.to_owned();
                let comment_raw = modal.data.components[1].components[0].value.to_owned();

                let game = if game_raw.len() == 0 {
                    None
                } else {
                    Some(game_raw)
                };

                let comment = if comment_raw.len() == 0 {
                    None
                } else {
                    Some(comment_raw)
                };

                self.create_new_mm_panel_from_modal(modal, game, comment, modal.guild_id.unwrap())
                    .await?;
            }
            _ => {
                warn!(modal_kind = %modal_kind, parts = %format!("{:?}", args), "Unknown modal_kind");
            }
        }

        Ok(())
    }

    async fn create_new_mm_panel_from_modal(
        &self,
        modal: &ModalSubmitInteraction,
        game: Option<String>,
        comment: Option<String>,
        guild_id: Id<GuildMarker>,
    ) -> Result<(), RunbackError> {
        let panel = entity::matchmaking::panel::Model {
            panel_id: Uuid::new_v4(),
            guild_id: guild_id.into(),
            message_id: None,
            channel_id: None,
            game,
            comment,
        };

        let res = entity::matchmaking::Panel::insert(panel.clone().into_active_model())
            .exec(self.utils.db_ref())
            .await?;

        debug!(res = %format!("{:?}", res), "Panel insert result");

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

        let panel_view = AdminLobbiesSinglePanel {
            panel: &panel,
            text_channels: text_channels.as_slice(),
        };

        let response = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(panel_view.create().build()),
        };

        let _res = self
            .utils
            .http_client
            .interaction(self.utils.application_id)
            .create_response(modal.id, modal.token.as_str(), &response)
            .exec()
            .await?;

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
            InteractionResponseDataBuilder::new()
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
                    // self.create_new_mm_panel(component).await?;
                    todo!("Send the mm admin panel");
                    // Send the new panel with the interaction data
                }

                let data = InteractionResponseDataBuilder::new()
                    .components(vec![
                        Component::ActionRow(ActionRow {
                            components: vec![Component::TextInput(TextInput {
                                custom_id: "admin:mm:panels:modal:game".to_owned(),
                                label: "Game title".to_string(),
                                max_length: Some(100),
                                min_length: None,
                                placeholder: Some(
                                    "Enter the title of the game that is related to this panel."
                                        .to_owned(),
                                ),
                                required: Some(false),
                                style: TextInputStyle::Short,
                                value: None,
                            })],
                        }),
                        Component::ActionRow(ActionRow {
                            components: vec![Component::TextInput(TextInput {
                                custom_id: "admin:mm:panels:modal:comment".to_owned(),
                                label: "Panel comment".to_string(),
                                max_length: Some(100),
                                min_length: None,
                                placeholder: Some(
                                    "Enter a comment that you want to attach to this panel."
                                        .to_owned(),
                                ),
                                required: Some(false),
                                style: TextInputStyle::Paragraph,
                                value: None,
                            })],
                        }),
                    ])
                    .title("Create a new Matchmaking Panel".to_owned())
                    .custom_id("admin:mm:panel:new".to_owned())
                    .build();
                let message = InteractionResponse {
                    kind: InteractionResponseType::Modal,
                    data: Some(data),
                };

                self.utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_response(component.id, component.token.as_str(), &message)
                    .exec()
                    .await?;

                // Otherwise, update the message with all of the fields for the new panel
            }
            "select" => {
                debug!(values = %format!("{:#?}", component.data.values));

                let panel = entity::matchmaking::Panel::find_by_id(
                    Uuid::parse_str(component.data.values[0].as_str()).unwrap(),
                )
                .one(self.utils.db_ref())
                .await?
                .unwrap();

                let channels = self
                    .utils
                    .http_client
                    .guild_channels(component.guild_id.unwrap())
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

                let panel_view = AdminLobbiesSinglePanel {
                    panel: &panel,
                    text_channels: text_channels.as_slice(),
                };
                let callback_data = panel_view.create();

                let message = InteractionResponse {
                    kind: InteractionResponseType::ChannelMessageWithSource,
                    data: Some(callback_data.build()),
                };

                let _res = self
                    .utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_response(component.id, component.token.as_str(), &message)
                    .exec()
                    .await?;
            }
            "delete" => {
                let panel =
                    entity::matchmaking::Panel::delete(entity::matchmaking::panel::ActiveModel {
                        panel_id: entity::sea_orm::Set(
                            Uuid::parse_str(component.data.custom_id.split(':').last().unwrap())
                                .unwrap(),
                        ),
                        ..Default::default()
                    })
                    .exec(self.utils.db_ref())
                    .await?;
                let message = InteractionResponse {
                    kind: InteractionResponseType::UpdateMessage,
                    data: Some(
                        InteractionResponseDataBuilder::new()
                            .content("Panel successfully deleted".into())
                            .components(vec![])
                            .build(),
                    ),
                };

                let _res = self
                    .utils
                    .http_client
                    .interaction(self.utils.application_id)
                    .create_response(component.id, component.token.as_str(), &message)
                    .exec()
                    .await?;

                // TODO: Update the original interaction if it exists.
            }
            "change" => match args[1] {
                "channel" => {
                    let new_channel_id = component.data.values.get(0).unwrap();
                    let mm_channel = Id::<ChannelMarker>::new(new_channel_id[1..].parse().unwrap());
                    let panel_id =
                        Uuid::parse_str(component.data.custom_id.split(':').last().unwrap())
                            .unwrap();

                    let panel_model = entity::matchmaking::panel::ActiveModel {
                        panel_id: Set(panel_id),
                        channel_id: Set(Some(mm_channel.into())),
                        ..Default::default()
                    };

                    let res = panel_model.update(self.utils.db_ref()).await?;

                    // Also query from discord to check and see if the message is still there
                    if res.message_id.is_some() {
                        // TODO: Repost the message in the correct spot
                    } else {
                        // Post the mm panel to that channel
                        let panel = MatchmakingPanel {
                            model: &res,
                            lfg: todo!(),
                            lobbies: todo!(),
                        };

                        let data = panel.components();

                        let r = self
                            .utils
                            .http_client
                            .create_message(mm_channel)
                            // .components(data.as_slice())
                            // .unwrap()
                            .embeds(&[panel.embed()])
                            .unwrap()
                            .exec()
                            .await?;

                        debug!(message = %format!("{:#?}", r));

                        let panel_model = entity::matchmaking::panel::ActiveModel {
                            panel_id: Set(panel_id),
                            message_id: Set(Some(r.model().await.unwrap().id.into())),
                            ..Default::default()
                        };
                        let final_model = panel_model.update(self.utils.db_ref()).await?;

                        let channels = self
                            .utils
                            .http_client
                            .guild_channels(component.guild_id.unwrap())
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

                        let admin_view = AdminLobbiesSinglePanel {
                            panel: &final_model,
                            text_channels: text_channels.as_slice(),
                        };

                        let update_message_res = self
                            .utils
                            .http_client
                            .interaction(self.utils.application_id)
                            .create_response(
                                component.id,
                                component.token.as_str(),
                                &InteractionResponse {
                                    kind: InteractionResponseType::UpdateMessage,
                                    data: Some(
                                        InteractionResponseDataBuilder::new()
                                            .content(
                                                "Successfully changed the message channel"
                                                    .to_string(),
                                            )
                                            .components(admin_view.components())
                                            .build(),
                                    ),
                                },
                            )
                            .exec()
                            .await?;

                        debug!(res = %format!("{:#?}", update_message_res));
                    }
                }
                _ => {
                    warn!(arg = %args[1], "Unhandled argument found during \"change\" operation");
                }
            },
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
}
