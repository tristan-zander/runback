use std::error::Error;

use twilight_embed_builder::EmbedBuilder;
use twilight_model::{
    application::{
        callback::InteractionResponse,
        command::{Command, CommandType},
        component::{select_menu::SelectMenuOption, Component, SelectMenu},
        interaction::{
            application_command::{CommandDataOption, CommandOptionValue},
            ApplicationCommand as DiscordApplicationCommand,
        },
    },
    channel::message::MessageFlags,
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::{
    command::{CommandBuilder, SubCommandBuilder, SubCommandGroupBuilder},
    CallbackDataBuilder,
};

use super::ApplicationCommand;

pub(super) struct AdminCommandHandler;

impl ApplicationCommand for AdminCommandHandler {
    fn to_command(debug_guild: Option<Id<GuildMarker>>) -> Command {
        let mut builder = CommandBuilder::new(
            "admin".into(),
            "Admin configuration and management settings".into(),
            CommandType::ChatInput,
        )
        .option(SubCommandBuilder::new(
            "matchamking-settings".into(),
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
    pub async fn on_command_called(&self, command: &Box<DiscordApplicationCommand>) {
        let options = &command.data.options;

        // There should only be one subcommand option, but map through them anyways
        for option in options {
            match &option.value {
                CommandOptionValue::SubCommand(subcommand) => {
                    for submenu_command in subcommand {
                        match submenu_command.name.as_str() {
                            "matchmaking-settings" => {
                                self.send_matchamking_settings(command).await;
                                return;
                            }
                            _ => {
                                debug!(name = %submenu_command.name.as_str(), "Unknown admin subcommand option")
                            }
                        }
                    }
                }
                _ => {
                    debug!(name = %option.name.as_str(), option = %format!("{:?}", &option.value), "Called admin command with an unknown value.")
                }
            }
        }
    }

    pub async fn send_matchamking_settings(&self, command: &Box<DiscordApplicationCommand>) {
        let message = InteractionResponse::ChannelMessageWithSource(
            CallbackDataBuilder::new()
                .flags(MessageFlags::EPHEMERAL)
                .components(vec![Component::SelectMenu(SelectMenu {
                    custom_id: "matchmaking_settings__matchmaking_channel".into(),
                    disabled: false,
                    max_values: Some(1),
                    min_values: Some(1),
                    options: vec![
                        // TODO: fill this out based on channels that exist in the guild
                        SelectMenuOption {
                            default: false,
                            description: Some("Matchmaking Channel".into()),
                            emoji: None,
                            label: "#matchmaking".into(),
                            // Fake Discord channel ID
                            value: "1234567812345678".into(),
                        },
                    ],
                    // TODO: Get this from the DB or cache
                    placeholder: None,
                })])
                .build(),
        );
    }

    pub async fn handle_matchmaking_settings_changed() {}
}
