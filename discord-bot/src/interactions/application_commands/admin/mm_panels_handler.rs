use std::sync::Arc;

use entity::{
    sea_orm::{ColumnTrait, EntityTrait, QueryFilter},
    IdWrapper,
};
use twilight_model::{
    channel::{Channel, ChannelType},
    http::interaction::{InteractionResponse, InteractionResponseType},
    id::marker::GuildMarker,
};

use crate::interactions::{
    application_commands::{ApplicationCommandUtilities, InteractionData},
    panels::admin_lobby::AdminLobbiesPanel,
};

pub struct MatchmakingPanelsHandler;

impl MatchmakingPanelsHandler {
    pub async fn execute(
        utils: Arc<ApplicationCommandUtilities>,
        data: Box<InteractionData>,
    ) -> anyhow::Result<()> {
        let command = &data.command;
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err(anyhow!("Command was not run in a guild"));
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

        let panels = entity::matchmaking::Panel::find()
            .filter(
                entity::matchmaking::panel::Column::GuildId
                    .eq(Into::<IdWrapper<GuildMarker>>::into(guild_id)),
            )
            .all(utils.db_ref())
            .await?;

        let panel = AdminLobbiesPanel {
            guild_id,
            text_channels: text_channels.as_slice(),
            panels: panels.as_slice(),
        };

        let callback_data = panel.create();

        let message = InteractionResponse {
            kind: InteractionResponseType::ChannelMessageWithSource,
            data: Some(callback_data.build()),
        };

        utils
            .send_message(command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?;

        // let token = command.token.clone();
        // let mut events = self
        //     .utils
        //     .standby
        //     .wait_for_stream(guild_id, move |e: &Event| -> bool {
        //         debug!("Standby event: {:#?}", e);
        //         match e {
        //             twilight_gateway::Event::InteractionCreate(interaction) => match &interaction.0
        //             {
        //                 twilight_model::application::interaction::Interaction::MessageComponent(
        //                     comp,
        //                 ) => comp.token == token,
        //                 twilight_model::application::interaction::Interaction::ModalSubmit(
        //                     modal,
        //                 ) => modal.token == token,
        //                 _ => false,
        //             },
        //             _ => false,
        //         }
        //     });

        // while let Some(ev) = events.next().await {
        //     match ev {
        //         Event::InteractionCreate(interaction) => {
        //             match &interaction.0 {
        //                 twilight_model::application::interaction::Interaction::MessageComponent(
        //                     component,
        //                 ) => {
        //                     info!("Recieved message component interaction while awaiting standby.");
        //                 }
        //                 twilight_model::application::interaction::Interaction::ModalSubmit(
        //                     modal,
        //                 ) => {
        //                     todo!()
        //                 }
        //                 _ => {}
        //             }
        //             // self.on_message_component_event(id_parts, component);
        //         }
        //         _ => {}
        //     }
        // }

        info!("Finished handling mm_panels event");

        Ok(())
    }
}
