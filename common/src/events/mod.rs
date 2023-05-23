use cqrs_es::DomainEvent;



#[derive(Serialize, Deserialize)]
pub enum LobbyCommand {
    OpenLobby { },
    CloseLobby { },
    AddPlayerToLobby { player_id: u64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LobbyEvent {
    LobbyOpened {},
    LobbyClosed {},
    PlayerAddedToLobby { player_id: u64 }
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

impl std::fmt::Display for LobbyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for LobbyError {}