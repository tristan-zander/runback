use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

pub use active_session::Entity as ActiveSession;
pub use panel::Entity as Panel;
pub use settings::Entity as Setting;

pub mod active_session {
    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "matchmaking_sessions")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: i32,
        pub started_at: DateTimeUtc,
        /// Usually set to 60 minutes after the match started, depending on whether the players need more time.
        /// The max value for this field is 3 hours
        pub timeout_after: DateTimeUtc,
        /// Sometimes, you may not want to start a thread with a match. That's up to either the admins or the user
        pub thread_id: Option<i64>,
        // pub game: Id for GameMetadata,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_many = "crate::discord_user::user::Entity")]
        User,
        // #[sea_orm(has_one)]
        // GameMetadata
    }

    impl Related<crate::discord_user::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::User.def()
        }
    }

    impl ActiveModelBehavior for ActiveModel {}
}

/// MM settings are set one per server
pub mod settings {
    use chrono::Utc;
    use twilight_model::id::{
        marker::{ChannelMarker, GuildMarker},
        Id,
    };

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
    #[sea_orm(table_name = "matchmaking_settings")]
    pub struct Model {
        #[sea_orm(primary_key, unique, auto_increment = false)]
        pub guild_id: i64,
        #[sea_orm(default_value = Utc::now())]
        pub last_updated: DateTimeUtc,
        /// Set the date that a guild admin accepted the EULA
        // TODO: Make a general Guild table and put this field there
        pub has_accepted_eula: Option<DateTimeUtc>,
        /// The channel ID for where the matchmaking panel should be posted.
        pub channel_id: Option<i64>,
        #[sea_orm(default_value = false)]
        pub threads_are_private: bool,
    }

    impl Default for Model {
        fn default() -> Self {
            Self {
                guild_id: 0,
                last_updated: Utc::now(),
                has_accepted_eula: None,
                channel_id: None,
                threads_are_private: false,
            }
        }
    }

    impl Model {
        // pub fn cast_channel_id(&self) -> Option<Id<ChannelMarker>> {
        //     if let Some(cid) = self.channel_id {
        //         return Some(Id::new(cid));
        //     }
        //     None
        // }

        // pub fn cast_guild_id(&self) -> Id<GuildMarker> {
        //     Id::new(self.guild_id)
        // }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}

    impl ActiveModelBehavior for ActiveModel {}
}

/// The matchmaking panel that users can interact with
pub mod panel {
    use twilight_model::id::{marker::GuildMarker, Id};

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
    #[sea_orm(table_name = "matchmaking_panels")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub guild_id: i64,
        pub message_id: i64,
        pub game: String,
    }

    impl Model {
        // pub fn cast_guild_id(&self) -> Id<GuildMarker> {
        //     Id::new(self.guild_id)
        // }
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        // ActiveSession,
    // LookingForMatch
    }

    impl ActiveModelBehavior for ActiveModel {}
}
