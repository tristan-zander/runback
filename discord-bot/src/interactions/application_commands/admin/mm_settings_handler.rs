use std::sync::Arc;

use twilight_model::{
    application::component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
    channel::{message::MessageFlags, Channel, ChannelType},
    http::interaction::{InteractionResponse, InteractionResponseType},
};
use twilight_util::builder::InteractionResponseDataBuilder;

use crate::interactions::application_commands::{
    ApplicationCommandHandler, ApplicationCommandUtilities, CommandHandlerType, InteractionData,
};

pub struct MatchmakingSettingsHandler {
    pub utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl ApplicationCommandHandler for MatchmakingSettingsHandler {
    fn name(&self) -> String {
        "matchmaking-settings".to_string()
    }

    fn register(&self) -> CommandHandlerType {
        CommandHandlerType::SubCommand
    }

    async fn execute(&self, data: &InteractionData) -> anyhow::Result<()> {
        let command = data.command;
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

        Ok(self
            .utils
            .send_message(command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?)
    }
}
