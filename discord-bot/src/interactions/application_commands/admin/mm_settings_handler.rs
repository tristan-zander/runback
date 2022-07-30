use std::sync::Arc;

use chrono::Utc;
use entity::sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel};
use twilight_model::{
    application::{
        component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
        interaction::MessageComponentInteraction,
    },
    channel::{message::MessageFlags, Channel, ChannelType},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::{marker::ChannelMarker, Id},
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::interactions::application_commands::{
    ApplicationCommandData, ApplicationCommandUtilities, CommandGroupDescriptor,
    InteractionHandler, MessageComponentData,
};

pub struct MatchmakingSettingsHandler {
    utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl InteractionHandler for MatchmakingSettingsHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        // This is not a top-level command handler.
        // This function should never be registered into the InteractionProcessor/
        CommandGroupDescriptor {
            name: "settings",
            description: "View/update admin matchmaking settings",
            commands: Box::new([]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        let command = &data.command;
        // VERIFY: Is it possible that we can send the information of other guilds here?
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err(anyhow!("Can't find a guild id for this command."));
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
                InteractionResponseDataBuilder::new()
                    .flags(MessageFlags::EPHEMERAL)
                    .components(vec![Component::ActionRow(ActionRow {
                        components: vec![Component::SelectMenu(SelectMenu {
                            custom_id: "admin:settings:channel".into(),
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

        self.utils
            .send_message(command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?;

        Ok(())
    }

    async fn process_autocomplete(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!()
    }

    async fn process_modal(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!()
    }

    async fn process_component(&self, data: Box<MessageComponentData>) -> anyhow::Result<()> {
        match data.action.as_str() {
            "channel" => {
                self.set_matchmaking_channel(&data.message).await?;
                return Ok(());
            }
            _ => {
                return Err(anyhow!(
                    "Unknown field given to admin settings: {}",
                    &data.action
                ))
            }
        }
    }
}

impl MatchmakingSettingsHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self { utils }
    }

    async fn set_matchmaking_channel(
        &self,
        component: &MessageComponentInteraction,
    ) -> anyhow::Result<()> {
        let channel_id: Id<ChannelMarker> = Id::new(
            component
                .data
                .values
                .get(0)
                .ok_or_else(|| anyhow!("No component values provided."))?
                .parse::<u64>()
                .map_err(|e| anyhow!(e))?,
        );

        let guild_id = component
            .guild_id
            .ok_or_else(|| anyhow!("You cannot use Runback in a DM."))?;

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
        let _message = InteractionResponse { kind: InteractionResponseType::UpdateMessage, data: Some(
            InteractionResponseDataBuilder::new()
                .flags(MessageFlags::EPHEMERAL)
                .content("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect.".into())
                .build()
        )};

        let _res =
            self.utils
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
}
