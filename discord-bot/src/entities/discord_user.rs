use super::*;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use user::Entity as User;

/// The matchmaking panel that users can interact with
pub mod user {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "users")]
    pub struct Model {
        #[sea_orm(primary_key)]
        /// Discord user id
        pub user_id: u64,
        pub active_session: i32,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(
            belongs_to = "crate::entities::matchmaking::active_session::Entity",
            from = "Column::ActiveSession",
            to = "crate::entities::matchmaking::active_session::Column::Id"
        )]
        Session,
    }

    impl Related<crate::entities::matchmaking::active_session::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::Session.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}
