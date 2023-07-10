use crate::{entity::IdWrapper, events::Lobby};
use chrono::{DateTime, Utc};
use cqrs_es::{persist::GenericQuery, EventEnvelope, View};
use sea_orm::entity::prelude::*;
use sea_orm::DeriveEntityModel;
use serde::{Deserialize, Serialize};
use twilight_model::id::{
    marker::{ChannelMarker, UserMarker},
    Id,
};

pub use Model as LobbyView;

use super::{MaterializedViewTrait, SeaOrmViewRepository};

// TODO: Build custom query that automatically updates the materialized views as events come in.
pub type LobbyQuery =
    GenericQuery<SeaOrmViewRepository<LobbyView, Lobby, ActiveModel>, LobbyView, Lobby>;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "lobbies")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub owner: IdWrapper<UserMarker>,
    #[sea_orm(ignore)]
    pub players: Vec<IdWrapper<UserMarker>>,
    pub opened: DateTime<Utc>,
    pub closed: Option<DateTime<Utc>>,
    pub channel: IdWrapper<ChannelMarker>,
    pub version: u64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}

impl MaterializedViewTrait for Model {
    type VersionColumnType = Column;

    fn version_column() -> Self::VersionColumnType {
        Column::Version
    }
}

impl Default for LobbyView {
    fn default() -> Self {
        Self {
            /// SAFETY: Discord will throw us an error if it's passesd an Id of 0.
            /// These IDs are required fields so it's almost guaranteed to be replaced by a real value.
            owner: unsafe { Id::new_unchecked(0).into() },
            players: Default::default(),
            opened: Default::default(),
            closed: Default::default(),
            channel: unsafe { Id::new_unchecked(0).into() },
            version: 0,
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
                self.owner = owner_id.into();
                self.channel = channel_id.into();
                self.players.push(owner_id.into());
            }
            crate::events::LobbyEvent::LobbyClosed { at } => {
                self.closed = Some(at);
            }
            crate::events::LobbyEvent::PlayerAddedToLobby { player_id } => {
                self.players.push(player_id.into());
            }
        }
    }
}
