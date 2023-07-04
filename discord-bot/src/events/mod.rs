use chrono::{DateTime, Utc};
use cqrs_es::{Aggregate, DomainEvent};
use serde::{Deserialize, Serialize};

use crate::services::LobbyService;

// Lobby aggregate
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Lobby {
    owner_id: String,
    players: Vec<String>,
    opened: DateTime<Utc>,
    closed: Option<DateTime<Utc>>,
    channel: u64,
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
                self.owner_id = owner_id.to_string();
                self.channel = channel_id;
                self.opened = Utc::now();
                self.players.push(owner_id.to_string());
            }
            LobbyEvent::LobbyClosed { at } => {
                self.closed = Some(at);
            }
            LobbyEvent::PlayerAddedToLobby { player_id } => {
                self.players.push(player_id.to_string());
            }
            _ => {
                panic!("Unhandled CRQS event {:?}", event)
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum LobbyCommand {
    OpenLobby { owner_id: u64, channel: u64 },
    CloseLobby {},
    AddPlayerToLobby { player_id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LobbyEvent {
    LobbyOpened { owner_id: u64, channel_id: u64 },
    LobbyClosed { at: DateTime<Utc> },
    PlayerAddedToLobby { player_id: u64 },
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
