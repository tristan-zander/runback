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
    application_commands::{
        ApplicationCommandData, ApplicationCommandUtilities, CommandGroupDescriptor,
        InteractionHandler, MessageComponentData,
    },
};

pub struct MatchmakingPanelsHandler {
    utils: Arc<ApplicationCommandUtilities>,
}

#[async_trait]
impl InteractionHandler for MatchmakingPanelsHandler {
    fn describe(&self) -> CommandGroupDescriptor {
        // This is not a top-level command handler.
        // This function should never be registered into the InteractionProcessor/
        CommandGroupDescriptor {
            name: "panels",
            description: "Create/manage matchmaking panels",
            commands: Box::new([]),
        }
    }

    async fn process_command(&self, data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        let command = &data.command;
        let guild_id = match command.guild_id {
            Some(id) => id,
            None => {
                return Err(anyhow!("Command was not run in a guild"));
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

        let panels = entity::matchmaking::Panel::find()
            .filter(
                entity::matchmaking::panel::Column::GuildId
                    .eq(Into::<IdWrapper<GuildMarker>>::into(guild_id)),
            )
            .all(self.utils.db_ref())
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

        self.utils
            .send_message(command, &message)
            .await
            .map_err(|e| anyhow!("Could not send message: {}", e))?;

        info!("Finished handling mm_panels event");

        Ok(())
    }

    async fn process_autocomplete(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!()
    }

    async fn process_modal(&self, _data: Box<ApplicationCommandData>) -> anyhow::Result<()> {
        todo!()
    }

    async fn process_component(&self, _data: Box<MessageComponentData>) -> anyhow::Result<()> {
        todo!()
    }
}

impl MatchmakingPanelsHandler {
    pub fn new(utils: Arc<ApplicationCommandUtilities>) -> Self {
        Self { utils }
    }
}
