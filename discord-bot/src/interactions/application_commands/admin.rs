use std::{error::Error, sync::Arc};

use twilight_embed_builder::EmbedBuilder;
use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::{Command, CommandType},
        component::{select_menu::SelectMenuOption, ActionRow, Component, SelectMenu},
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand as DiscordApplicationCommand, MessageComponentInteraction,
        },
    },
    channel::{message::MessageFlags, GuildChannel, TextChannel},
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder, SubCommandGroupBuilder},
    CallbackDataBuilder,
};

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
    pub command_utils: Arc<ApplicationCommandUtilities>,
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
        Self { command_utils }
    }

    pub async fn on_command_called(&self, command: &Box<DiscordApplicationCommand>) {
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
    ) -> Result<(), Box<dyn Error>> {
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
    ) -> Result<(), Box<dyn Error>> {

        Ok(())
    }

    pub async fn send_matchamking_settings(
        &self,
        command: &Box<DiscordApplicationCommand>,
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
            .command_utils
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

        Ok(self.command_utils.send_message(command, &message).await?)
    }

    pub async fn handle_matchmaking_settings_changed() {}
}
