use chrono::{DateTime, Utc};
use cqrs_es::{Aggregate, DomainEvent};
use serde::{Deserialize, Serialize};
use twilight_model::id::{
    marker::{ChannelMarker, UserMarker},
    Id,
};

use crate::services::LobbyService;

// Lobby aggregate
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Lobby {
    owner_id: Id<UserMarker>,
    players: Vec<Id<UserMarker>>,
    opened: DateTime<Utc>,
    closed: Option<DateTime<Utc>>,
    channel: Id<ChannelMarker>,
}

impl Default for Lobby {
    fn default() -> Self {
        Self {
            /// SAFETY: This aggregate will never be spawned without proper owner and channel Ids
            owner_id: unsafe { Id::new_unchecked(0) },
            channel: unsafe { Id::new_unchecked(0) },
            players: vec![],
            opened: Utc::now(),
            closed: None,
        }
    }
}

#[async_trait]
impl Aggregate for Lobby {
    type Command = LobbyCommand;
    type Event = LobbyEvent;
    type Error = LobbyError;
    type Services = LobbyService;

    fn aggregate_type() -> String {
        stringify!(Lobby).to_string()
    }

    async fn handle(
        &self,
        command: Self::Command,
        services: &Self::Services,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            LobbyCommand::OpenLobby { owner_id, channel } => {
                services.open_lobby(owner_id, channel).await?;
                return Ok(vec![LobbyEvent::LobbyOpened {
                    owner_id,
                    channel_id: channel,
                }]);
            }
            LobbyCommand::CloseLobby {} => {
                return Ok(vec![LobbyEvent::LobbyClosed { at: Utc::now() }]);
            }
            LobbyCommand::AddPlayerToLobby { player_id } => {
                return Ok(vec![LobbyEvent::PlayerAddedToLobby { player_id }]);
            }
            _ => {
                panic!("Unhandled lobby command.");
            }
        }
    }

    fn apply(&mut self, event: Self::Event) {
        match event {
            LobbyEvent::LobbyOpened {
                owner_id,
                channel_id,
            } => {
                self.owner_id = owner_id;
                self.channel = channel_id;
                self.opened = Utc::now();
                self.players.push(owner_id);
            }
            LobbyEvent::LobbyClosed { at } => {
                self.closed = Some(at);
            }
            LobbyEvent::PlayerAddedToLobby { player_id } => {
                self.players.push(player_id);
            }
            _ => {
                panic!("Unhandled CRQS event {:?}", event)
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum LobbyCommand {
    OpenLobby {
        owner_id: Id<UserMarker>,
        channel: Id<ChannelMarker>,
    },
    CloseLobby {},
    AddPlayerToLobby {
        player_id: Id<UserMarker>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LobbyEvent {
    LobbyOpened {
        owner_id: Id<UserMarker>,
        channel_id: Id<ChannelMarker>,
    },
    LobbyClosed {
        at: DateTime<Utc>,
    },
    PlayerAddedToLobby {
        player_id: Id<UserMarker>,
    },
}

impl DomainEvent for LobbyEvent {
    fn event_type(&self) -> String {
        format!("{:?}", self)
    }

    fn event_version(&self) -> String {
        "1.0.0".to_string()
    }
}

#[derive(Debug)]
pub struct LobbyError(String);

impl From<&str> for LobbyError {
    fn from(value: &str) -> Self {
        LobbyError(value.to_string())
    }
}

impl From<String> for LobbyError {
    fn from(value: String) -> Self {
        LobbyError(value)
    }
}

impl std::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LobbyError {}
