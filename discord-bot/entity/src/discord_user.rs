use super::*;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use user::Entity as User;

/// The matchmaking panel that users can interact with
pub mod user {
    use twilight_model::id::marker::UserMarker;

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        /// Discord user id
        pub user_id: IdWrapper<UserMarker>,
        pub active_session: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "crate::matchmaking::lobby::Entity",
            from = "Column::ActiveSession",
            to = "crate::matchmaking::lobby::Column::Id"
        )]
        Session,
        #[sea_orm(
            belongs_to = "crate::matchmaking::lfg::Entity",
            from = "Column::UserId",
            to = "crate::matchmaking::lfg::Column::UserId"
        )]
        LookingForGames,
    }

    impl Related<crate::matchmaking::lobby::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Session.def()
        }
    }

    impl Related<crate::matchmaking::lfg::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::LookingForGames.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
