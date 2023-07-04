//! This module contains core business logic for interacting with Runback's system.
//! Services generate events, which represent actions on behalf of the user.
//!
//! Any common business logic should exist as a service.

use std::num::NonZeroU64;

use twilight_http::Client;
use twilight_model::{
    channel::message::allowed_mentions::AllowedMentionsBuilder,
    id::{
        marker::{ChannelMarker, UserMarker},
        Id,
    },
};
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder};

use crate::events::*;

/// Contains core logic for interacting with a lobby.
pub struct LobbyService {
    discord: Client,
}

impl LobbyService {
    pub fn new(client: Client) -> Self {
        Self { discord: client }
    }

    #[instrument(skip(self))]
    pub async fn open_lobby(&self, owner_id: u64, channel_id: u64) -> Result<(), LobbyError> {
        let channel_id = Id::from(
            NonZeroU64::new(channel_id)
                .ok_or_else(|| LobbyError::from("Channel Id was found to be zero."))?,
        );

        let owner_id = Id::from(
            NonZeroU64::new(owner_id)
                .ok_or_else(|| LobbyError::from("Owner Id was found to be zero."))?,
        );

        let res = self
            .discord
            .create_thread(
                channel_id,
                "Matchmaking",
                twilight_model::channel::ChannelType::PublicThread,
            )
            .map_err(|e| LobbyError::from(format!("Could not create thread: {}", e.to_string())))?
            .await
            .map_err(|e| LobbyError::from(e.to_string()))?
            .model()
            .await
            .map_err(|e| LobbyError::from(e.to_string()))?;

        debug!(
            owner_id = owner_id.get(),
            channel_id = channel_id.get(),
            thread_id = res.id.get(),
            "Opened a public lobby thread"
        );

        self.send_thread_opening_message([owner_id], res.id)
            .await
            .map_err(|e| LobbyError::from(e.to_string()))?;

        Ok(())
    }

    pub async fn close_lobby(&self) -> Result<(), LobbyError> {
        unimplemented!();
    }

    pub async fn add_player_to_lobby(&self, _lobby: Lobby) -> Result<(), LobbyError> {
        unimplemented!();
    }

    async fn send_thread_opening_message(
        &self,
        users: impl IntoIterator<Item = Id<UserMarker>>,
        channel: Id<ChannelMarker>,
    ) -> anyhow::Result<()> {
        let _msg = self
            .discord
            .create_message(channel)
            .allowed_mentions(Some(
                &AllowedMentionsBuilder::new()
                    .user_ids(users.into_iter())
                    .build(),
            ))
            .embeds(&[EmbedBuilder::new()
                .description(
                    "**Thank you for using Runback. \
                        Below are a list of commands to assist you during your matches.**",
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking report",
                        "Report the score for your match",
                    )
                    .build(),
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking done",
                        "Finish matchmaking and finalize results",
                    )
                    .build(),
                )
                .field(
                    EmbedFieldBuilder::new(
                        "/matchmaking settings",
                        "Set the settings of the lobby.",
                    )
                    .build(),
                )
                .validate()?
                .build()])?
            .await?;

        Ok(())
    }
}
