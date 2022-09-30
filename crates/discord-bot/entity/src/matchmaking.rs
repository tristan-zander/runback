use sea_orm::entity::prelude::*;

use sea_orm::sea_query;

pub use lfg::Entity as LookingForGames;
pub use lobby::{Entity as Lobby, Model as LobbyModel};
pub use panel::Entity as Panel;
pub use settings::Entity as Setting;

/// Lobbies are sessions that users join to play matches. This could be open, closed, 1v1, team based, ffa, etc.
pub mod lobby {
    use twilight_model::id::marker::{ChannelMarker, UserMarker};

    use crate::IdWrapper;

    use super::*;

    #[derive(
        Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, EnumIter, DeriveActiveEnum, Iden,
    )]
    #[sea_orm(rs_type = "String", db_type = "Enum", enum_name = "lobby_privacy")]
    pub enum LobbyPrivacy {
        #[sea_orm(string_value = "Open")]
        Open,
        #[sea_orm(string_value = "InviteOnly")]
        InviteOnly,
    }

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveActiveModelBehavior)]
    #[sea_orm(table_name = "matchmaking_lobbies")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: Uuid,
        pub started_at: DateTimeUtc,
        /// Usually set to 60 minutes after the match started, depending on whether the players need more time.
        /// The max value for this field is 3 hours
        pub timeout_after: DateTimeUtc,
        /// Sometimes, you may not want to start a thread with a match. That's up to either the admins or the user
        pub thread_id: Option<IdWrapper<ChannelMarker>>,
        /// The privacy of the channel. Invite-Only lobbies will not be shown on the lobbies list
        pub privacy: LobbyPrivacy,
        /// The Discord member that "owns" the lobby
        pub owner: IdWrapper<UserMarker>,
        /// The message attached to the lobby.
        /// Max length of 80
        #[sea_orm(default_value = "No lobby description")]
        pub description: String,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        /// The current users that have entered the lobby
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
}

/// A User that's looking for a game in a certain category.
pub mod lfg {
    use twilight_model::id::marker::{GuildMarker, UserMarker};

    use crate::IdWrapper;

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveActiveModelBehavior)]
    #[sea_orm(table_name = "matchmaking_lfg")]
    pub struct Model {
        #[sea_orm(primary_key)]
        pub id: Uuid,
        #[sea_orm(unique)]
        pub guild_id: IdWrapper<GuildMarker>,
        pub user_id: IdWrapper<UserMarker>,
        pub started_at: DateTimeUtc,
        #[sea_orm(nullable)]
        pub timeout_after: Option<DateTimeUtc>,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {
        #[sea_orm(has_one = "crate::discord_user::user::Entity")]
        User,
    }

    impl Related<crate::discord_user::user::Entity> for Entity {
        fn to() -> RelationDef {
            Relation::User.def()
        }
    }
}

/// MM settings are set one per server
pub mod settings {
    use twilight_model::id::{
        marker::{ChannelMarker, GuildMarker, RoleMarker},
        Id,
    };

    use crate::IdWrapper;

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveActiveModelBehavior)]
    #[sea_orm(table_name = "matchmaking_settings")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub guild_id: IdWrapper<GuildMarker>,
        #[sea_orm(default_value = Utc::now())]
        pub last_updated: DateTimeUtc,
        /// Set the date that a guild admin accepted the EULA
        // TODO: Make a general Guild table and put this field there
        #[sea_orm(default_value = None)]
        pub has_accepted_eula: Option<DateTimeUtc>,
        /// The channel ID for where the matchmaking panel should be posted.
        #[sea_orm(default_value = None)]
        pub channel_id: Option<IdWrapper<ChannelMarker>>,
        #[sea_orm(default_value = None)]
        pub admin_role: Option<IdWrapper<RoleMarker>>,
        #[sea_orm(default_value = false)]
        pub threads_are_private: bool,
    }

    #[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
    pub enum Relation {}
}

/// The matchmaking panel that users can interact with
pub mod panel {
    use twilight_model::id::marker::{ChannelMarker, GuildMarker, MessageMarker};

    use crate::IdWrapper;

    use super::*;

    #[derive(Clone, Debug, PartialEq, DeriveEntityModel, DeriveActiveModelBehavior)]
    #[sea_orm(table_name = "matchmaking_panels")]
    pub struct Model {
        #[sea_orm(primary_key, auto_increment = false)]
        pub panel_id: Uuid,
        pub guild_id: IdWrapper<GuildMarker>,
        #[sea_orm(unique, nullable)]
        pub message_id: Option<IdWrapper<MessageMarker>>,
        #[sea_orm(unique, nullable)]
        pub channel_id: Option<IdWrapper<ChannelMarker>>,

        /// 80 Character Game Title
        #[sea_orm(nullable)]
        pub game: Option<String>,

        /// 255 Character Game Title
        #[sea_orm(nullable)]
        pub comment: Option<String>,
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
}
