use twilight_model::{
    application::component::{
        button::ButtonStyle, select_menu::SelectMenuOption, ActionRow, Button, Component,
        SelectMenu,
    },
    channel::{embed::Embed, message::MessageFlags, Channel},
    id::{marker::GuildMarker, Id},
};
use twilight_util::builder::embed::*;
use twilight_util::builder::{embed::EmbedBuilder, InteractionResponseDataBuilder};

pub struct AdminViewAllPanel<'a> {
    pub guild_id: Id<GuildMarker>,
    pub text_channels: &'a [Channel],
    pub panels: &'a [entity::matchmaking::panel::Model],
}

impl<'a> AdminViewAllPanel<'a> {
    pub fn create(&self) -> InteractionResponseDataBuilder {
        let embed = EmbedBuilder::new()
            .title("Admin Panel")
            .description("Please select a panel.");

        InteractionResponseDataBuilder::new()
            .flags(MessageFlags::EPHEMERAL)
            .embeds([embed.build()])
            .components(self.components())
    }

    pub fn components(&self) -> Vec<Component> {
        let select_menu_options: Vec<_> = self
            .panels
            .iter()
            .filter_map(|p| {
                // let text_channel = self.text_channels
                //     .iter()
                //     .filter(|t| {
                //         debug!(channel = %format!("{:#?}", t));
                //         if let Some(chan) = &p.channel_id {
                //             return chan.inner.get() == t.id.get();
                //         }
                //         return false;
                //     })
                //     .collect::<Vec<_>>();

                // if text_channel.len() != 1 {
                //     warn!(id = %format!("{:?}", p.channel_id), channels = %format!("{:?}", text_channel), "Found multiple or no text channels by single id");
                //     return None;
                // }

                // let text_channel = text_channel[0];

                // let name = if let Some(n) = &text_channel.name {
                //     n.as_str()
                // } else {
                //     "Unknown Channel Name"
                // };

                Some(SelectMenuOption {
                    default: false,
                    description: p.comment.to_owned(),
                    emoji: None,
                    label: p.game.clone().unwrap_or("Unnamed Panel".into()),
                    value: p.panel_id.to_string(),
                })
            })
            .collect();

        let mut components = Vec::new();

        if select_menu_options.len() > 0 {
            let select_menu_row = Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: "admin:mm:panels:select".into(),
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

        components
    }
}

pub struct AdminViewSinglePanel<'a> {
    pub panel: &'a entity::matchmaking::panel::Model,
    pub text_channels: &'a [Channel],
}

impl<'a> AdminViewSinglePanel<'a> {
    pub fn create(&self) -> InteractionResponseDataBuilder {
        let game = self.panel.game.clone();
        let desc = self.panel.comment.clone();

        let embed = EmbedBuilder::new()
            .title(format!(
                "Manage \"{}\" panel.",
                game.unwrap_or("Unnamed".to_owned())
            ))
            .description(desc.unwrap_or("No description".to_owned()));

        InteractionResponseDataBuilder::new()
            .flags(MessageFlags::EPHEMERAL)
            .embeds([embed.build()])
            .components(self.components())
    }

    pub fn components(&self) -> Vec<Component> {
        vec![
            Component::ActionRow(ActionRow {
                components: vec![
                    Component::Button(Button {
                        custom_id: Some("admin:mm:panels:change:game".into()),
                        disabled: false,
                        emoji: None,
                        label: Some("Change Game".into()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("admin:mm:panels:change:comment".into()),
                        disabled: false,
                        emoji: None,
                        label: Some("Change Comment".into()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                ],
            }),
            Component::ActionRow(ActionRow {
                components: vec![Component::SelectMenu(SelectMenu {
                    custom_id: format!(
                        "{}:{}",
                        "admin:mm:panels:change:channel", self.panel.panel_id
                    ),
                    disabled: false,
                    max_values: Some(1),
                    min_values: Some(1),
                    options: self
                        .text_channels
                        .iter()
                        .map(|c| {
                            let default = if let Some(id) = &self.panel.channel_id {
                                id.inner.get() == c.id.get()
                            } else {
                                false
                            };

                            SelectMenuOption {
                                default,
                                description: None,
                                emoji: None,
                                label: format!(
                                    "#{}",
                                    c.name.as_ref().unwrap_or(&"unnamed-channel".into())
                                ),
                                value: format!("#{}", c.id),
                            }
                        })
                        .collect::<Vec<_>>(),
                    placeholder: Some("Change the channel that the panel is posted in".into()),
                })],
            }),
            Component::ActionRow(ActionRow {
                components: vec![
                    Component::Button(Button {
                        custom_id: Some("admin:mm:panels:repost".into()),
                        disabled: false,
                        emoji: None,
                        label: Some("Repost Message".into()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some(format!(
                            "{}:{}",
                            "admin:mm:panels:delete",
                            self.panel.panel_id.to_string()
                        )),
                        disabled: false,
                        emoji: None,
                        label: Some("Delete Panel".into()),
                        style: ButtonStyle::Danger,
                        url: None,
                    }),
                ],
            }),
        ]
    }
}

pub struct MatchmakingPanel<'a> {
    pub model: &'a entity::matchmaking::panel::Model,

    /// This should be a reference to all the sessions in the guild
    pub lfg: &'a [entity::matchmaking::lfg::Model],

    pub lobbies: &'a [entity::matchmaking::lobby::Model],
}

impl<'a> MatchmakingPanel<'a> {
    pub fn create(&self) -> InteractionResponseDataBuilder {
        InteractionResponseDataBuilder::new()
            .components(self.components())
            .embeds([self.embed()])
    }

    pub fn components(&self) -> Vec<Component> {
        vec![
            Component::ActionRow(ActionRow {
                components: vec![
                    Component::Button(Button {
                        custom_id: Some("mm_panel:start".to_string()),
                        disabled: true,
                        emoji: None,
                        label: Some("Start Matchmaking".to_string()),
                        style: ButtonStyle::Primary,
                        url: None,
                    }),
                    Component::Button(Button {
                        custom_id: Some("mm_panel:stop".to_string()),
                        disabled: true,
                        emoji: None,
                        label: Some("Stop Matchmaking".to_string()),
                        style: ButtonStyle::Danger,
                        url: None,
                    }),
                ],
            }),
            Component::ActionRow(ActionRow {
                components: vec![Component::Button(Button {
                    custom_id: Some("mm_panel:show_settings".to_string()),
                    disabled: true,
                    emoji: None,
                    label: Some("Edit your preferences".to_string()),
                    style: ButtonStyle::Primary,
                    url: None,
                })],
            }),
        ]
    }

    pub fn embed(&self) -> Embed {
        let mut builder = EmbedBuilder::new();

        builder = builder.author(EmbedAuthorBuilder::new("Runback".to_string()).build());

        builder = builder.title(
            self.model
                .game
                .clone()
                .unwrap_or("Unnamed Matchmaking Panel".to_string()),
        );

        builder = builder.description(
            self.model
                .comment
                .clone()
                .unwrap_or("Start matchmaking with other players!".to_string()),
        );

        return builder.build();
    }
}
