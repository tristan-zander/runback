use chrono::{DateTime, Utc};
use cqrs_es::{persist::GenericQuery, Aggregate, EventEnvelope, Query, View};
use postgres_es::PostgresViewRepository;
use serde::{Deserialize, Serialize};
use twilight_model::id::{
    marker::{ChannelMarker, UserMarker},
    Id,
};

use crate::events::Lobby;

pub struct DiscordEventQuery {}

#[async_trait]
impl<T: Aggregate + Serialize> Query<T> for DiscordEventQuery {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<T>]) {
        for event in events {
            debug!(
                "Received event: {} {}",
                aggregate_id,
                serde_json::to_string(&event.payload).unwrap()
            );
        }
    }
}

pub type LobbyQuery = GenericQuery<PostgresViewRepository<LobbyView, Lobby>, LobbyView, Lobby>;

#[derive(Debug, Serialize, Deserialize)]
pub struct LobbyView {
    pub owner: Id<UserMarker>,
    pub players: Vec<Id<UserMarker>>,
    pub opened: DateTime<Utc>,
    pub closed: Option<DateTime<Utc>>,
    pub channel: Id<ChannelMarker>,
}

impl Default for LobbyView {
    fn default() -> Self {
        Self {
            /// SAFETY: Discord will throw us an error if it's passesd an Id of 0.
            /// These IDs are required fields so it's almost guaranteed to be replaced by a real value.
            owner: unsafe { Id::new_unchecked(0) },
            players: Default::default(),
            opened: Default::default(),
            closed: Default::default(),
            channel: unsafe { Id::new_unchecked(0) },
        }
    }
}

impl View<Lobby> for LobbyView {
    fn update(&mut self, event: &EventEnvelope<Lobby>) {
        match event.payload {
            crate::events::LobbyEvent::LobbyOpened {
                owner_id,
                channel_id,
            } => {
                self.owner = Id::new(owner_id);
                self.channel = Id::new(channel_id);
                self.players.push(self.owner);
            }
            crate::events::LobbyEvent::LobbyClosed { at } => {
                self.closed = Some(at);
            }
            crate::events::LobbyEvent::PlayerAddedToLobby { player_id } => {
                self.players.push(Id::new(player_id));
            }
        }
    }
}
