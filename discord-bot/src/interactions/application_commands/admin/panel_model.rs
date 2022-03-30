use entity::sea_orm::prelude::Uuid;
use twilight_model::{
    application::component::{ActionRow, Component, SelectMenu},
    id::{
        marker::{ChannelMarker, MessageMarker},
        Id,
    },
};

pub struct PanelForm {
    pub panel_id: Option<Uuid>,
    pub message_id: Option<Id<MessageMarker>>,
    pub channel_id: Option<Id<ChannelMarker>>,
    pub game: Option<String>,
}

impl PanelForm {
    /// Creates the form for the user to create a panel with
    pub fn new_form() -> Component {
        let ret_component = Component::ActionRow(ActionRow {
            components: vec![Component::SelectMenu(SelectMenu {
                custom_id: "message_id".to_owned(),
                disabled: false,
                max_values: Some(1),
                min_values: Some(1),
                options: vec![],
                placeholder: Some("Channel".to_owned()),
            })],
        });

        ret_component
    }

    pub fn validate(&self) {}
}
