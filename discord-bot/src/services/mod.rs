//! This module contains core business logic for interacting with Runback's system.
//! Services generate events, which represent actions on behalf of the user.
//!
//! Any common business logic should exist as a service.

use twilight_http::Client;

use crate::events::*;

/// Contains core logic for interacting with a lobby.
pub struct LobbyService {
    discord: Client,
}

impl LobbyService {
    pub fn new(client: Client) -> Self {
        Self { discord: client }
    }

    pub async fn open_lobby(&self, owner_id: u64, channel_id: u64) -> Result<(), LobbyError> {
        return Ok(());
        unimplemented!();
    }

    pub async fn close_lobby(&self) -> Result<(), LobbyError> {
        return Ok(());
        unimplemented!();
    }

    pub async fn add_player_to_lobby(&self, lobby: Lobby) -> Result<(), LobbyError> {
        return Ok(());
        unimplemented!();
    }
}
