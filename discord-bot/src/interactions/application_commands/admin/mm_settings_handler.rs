use std::{sync::Arc, time::Duration};

use chrono::Utc;
use entity::sea_orm::{EntityTrait, IntoActiveModel, ActiveModelTrait};
use futures::StreamExt;
use tokio::time::error::Elapsed;
use twilight_gateway::Event;
use twilight_http::request;
use twilight_model::{
    application::{
        component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
        interaction::{Interaction, InteractionType, MessageComponentInteraction},
    },
    channel::{message::MessageFlags, Channel, ChannelType},
    http::interaction::{InteractionResponse, InteractionResponseType}, id::{Id, marker::ChannelMarker},
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::{interactions::application_commands::{ApplicationCommandUtilities, InteractionData}, error::RunbackError};

pub struct MatchmakingSettingsHandler;

impl MatchmakingSettingsHandler {
    pub async fn execute(
        utils: Arc<ApplicationCommandUtilities>,
        data: Box<InteractionData>,
    ) -> anyhow::Result<()> {
        let command = &data.command;
        // VERIFY: Is it possible that we can send the information of other guilds here?
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err(anyhow!("Can't find a guild id for this command."));
            }
        };

        let channels = utils
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

        utils
            .send_message(command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?;

        let application_id = utils.application_id;

        // TODO: Prefer to use InteractionHandler.process_component()
        let mut component_stream = utils
            .standby
            .wait_for_stream(guild_id, move |e: &Event|  
                match e {
                Event::InteractionCreate(int) => {
                    if int.application_id() != application_id {
                        return false;
                    }
                    int.0.kind() == InteractionType::MessageComponent
                }
                _ => false,
            });

        let timeout : Result<anyhow::Result<()>, Elapsed> = tokio::time::timeout(Duration::from_secs(60 * 10), async {
            while let Some(e) = component_stream.next().await {
                match e {
                    Event::InteractionCreate(int) => match int.0 {
                        Interaction::MessageComponent(component) => {
                            if component.data.custom_id == "admin:mm:channel" {
                                Self::set_matchmaking_channel(utils.clone(), component.as_ref()).await?;
                            }
                        }
                        _ => unreachable!("The standby should always filter out non-components"),
                    },
                    _ => unreachable!("The standby should never give us this type of interactions"),
                }
            }
            Ok(())
        })
        .await;

        if let Err(_) = timeout {
            // TODO: Investigate why this doesn't work
            debug!("Time to timeout!");
            utils.http_client.interaction(utils.application_id).delete_response(command.token.as_str()).exec().await?;
        }

        Ok(())
    }

    async fn set_matchmaking_channel(
        utils: Arc<ApplicationCommandUtilities>,
        component: &MessageComponentInteraction,
    ) -> anyhow::Result<()> {
        let channel_id: Id<ChannelMarker> = Id::new(
            component
                .data
                .values
                .get(0)
                .ok_or_else(|| anyhow!("No component values provided."))?
                .parse::<u64>()
                .map_err(|e| {
                    anyhow!(e)
                })?,
        );

        let guild_id = component
            .guild_id
            .ok_or_else(|| anyhow!("You cannot use Runback in a DM."))?;

        let setting = entity::matchmaking::Setting::find_by_id(guild_id.into())
            .one(utils.db_ref())
            .await?;

        let _setting = if setting.is_some() {
            let mut setting = unsafe { setting.unwrap_unchecked() }.into_active_model();
            setting.channel_id = entity::sea_orm::Set(Some(channel_id.into()));
            setting.update(utils.db_ref()).await?
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
                .insert(utils.db_ref())
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
            utils
            .http_client
            .interaction(utils.application_id)
            .update_response(component.token.as_str())
            .content(Some("Successfully set the matchmaking channel. Please wait a few moments for changes to take effect"))?
            // .map_err(|e| RunbackError { message: "Could not set content for response message during set_matchmaking_channel()".to_owned(), inner: Some(Box::new(e)) })?
            // .(component.id, component.token.as_str(), &message)
            .exec()
            .await?;

        Ok(())
    }
}
