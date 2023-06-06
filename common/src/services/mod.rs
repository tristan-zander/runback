//! This module contains core business logic for interacting with Runback's system.
//! Services generate events, which represent actions on behalf of the user.
//! 
//! Any common business logic should exist as a service.



use postgres_es::{PostgresCqrs, PostgresEventRepository};

use crate::events::*;

/// Contains core logic for interacting with a lobby.
pub struct LobbyService {
    // 
}

impl LobbyService {
    /// Opens a lobby for players to join.
    pub async fn open_lobby(&self, owner_id: u64) -> Result<(), LobbyError> {


        unimplemented!();

        Ok(())
    }
}