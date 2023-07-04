use crate::entity::{
    prelude::*, sea_orm::sea_query::extension::postgres::Type, sea_orm_active_enums::LobbyPrivacy,
};
use crate::migration::deprecated::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum LobbyType {
    LobbyPrivacy,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // let stmt = Statement::from_string(manager.get_database_backend(), "BEGIN".to_owned());
        // manager.get_connection().execute(stmt).await?;

        manager
            .create_type(
                Type::create()
                    .as_enum(LobbyType::LobbyPrivacy)
                    .values([LobbyPrivacy::Open, LobbyPrivacy::InviteOnly])
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Game)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(game::Column::Id)
                            .uuid()
                            .primary_key()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(game::Column::Name)
                            .string_len(80)
                            .not_null()
                            .unique_key(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MatchmakingSettings)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking_settings::Column::GuildId)
                            .big_integer()
                            .primary_key()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking_settings::Column::HasAcceptedEula)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(matchmaking_settings::Column::LastUpdated)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(matchmaking_settings::Column::ChannelId).big_integer())
                    .col(ColumnDef::new(matchmaking_settings::Column::AdminRole).big_integer())
                    .col(
                        ColumnDef::new(matchmaking_settings::Column::ThreadsArePrivate)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Users)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(users::Column::UserId)
                            .uuid()
                            .primary_key()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(users::Column::DiscordUser)
                            .big_integer()
                            .unique_key(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(GameCharacter)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(game_character::Column::Id)
                            .uuid()
                            .not_null()
                            .primary_key()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(game_character::Column::Name)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(game_character::Column::Game)
                            .uuid()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(GameCharacter, game_character::Column::Game)
                            .to(Game, game::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(State)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(state::Column::Id)
                            .uuid()
                            .primary_key()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(state::Column::Key).big_integer().not_null())
                    .col(ColumnDef::new(state::Column::Value).json())
                    .col(ColumnDef::new(state::Column::UserId).uuid())
                    .foreign_key(
                        ForeignKey::create()
                            .from(State, state::Column::UserId)
                            .to(Users, users::Column::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MatchmakingInvitation)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking_invitation::Column::Id)
                            .uuid()
                            .primary_key()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking_invitation::Column::InvitedBy)
                            .not_null()
                            .uuid(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                MatchmakingInvitation,
                                matchmaking_invitation::Column::InvitedBy,
                            )
                            .to(Users, users::Column::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(matchmaking_invitation::Column::Game).uuid())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MatchmakingInvitation, matchmaking_invitation::Column::Game)
                            .to(Game, game::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(matchmaking_invitation::Column::Description).string_len(255),
                    )
                    .col(ColumnDef::new(matchmaking_invitation::Column::MessageId).big_integer())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MatchmakingPlayerInvitation)
                    .if_not_exists()
                    .primary_key(
                        Index::create()
                            .col(matchmaking_player_invitation::Column::InvitedPlayer)
                            .col(matchmaking_player_invitation::Column::Invitation),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_invitation::Column::InvitedPlayer)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                MatchmakingPlayerInvitation,
                                matchmaking_player_invitation::Column::InvitedPlayer,
                            )
                            .to(Users, users::Column::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_invitation::Column::Invitation)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                MatchmakingPlayerInvitation,
                                matchmaking_player_invitation::Column::Invitation,
                            )
                            .to(MatchmakingInvitation, matchmaking_invitation::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_invitation::Column::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MatchmakingLobbies)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking_lobbies::Column::Id)
                            .uuid()
                            .primary_key()
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking_lobbies::Column::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking_lobbies::Column::TimeoutAfter)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking_lobbies::Column::ChannelId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(matchmaking_lobbies::Column::Description).string_len(255))
                    .col(
                        ColumnDef::new(matchmaking_lobbies::Column::Owner)
                            .uuid()
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(MatchmakingLobbies, matchmaking_lobbies::Column::Owner)
                            .to(Users, users::Column::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(matchmaking_lobbies::Column::Privacy)
                            .enumeration(
                                LobbyType::LobbyPrivacy,
                                [LobbyPrivacy::Open, LobbyPrivacy::InviteOnly],
                            )
                            .not_null(),
                    )
                    .col(ColumnDef::new(matchmaking_lobbies::Column::Game).uuid())
                    .foreign_key(
                        ForeignKey::create()
                            .from(MatchmakingLobbies, matchmaking_lobbies::Column::Game)
                            .to(Game, game::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(matchmaking_lobbies::Column::GameOther).string_len(80))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MatchmakingPlayerLobby)
                    .if_not_exists()
                    .primary_key(
                        Index::create()
                            .col(matchmaking_player_lobby::Column::Player)
                            .col(matchmaking_player_lobby::Column::Lobby),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_lobby::Column::Player)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .from(
                                MatchmakingPlayerLobby,
                                matchmaking_player_lobby::Column::Player,
                            )
                            .to(Users, users::Column::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_lobby::Column::Lobby)
                            .uuid()
                            .not_null()
                            .unique_key(),
                    )
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .from(
                                MatchmakingPlayerLobby,
                                matchmaking_player_lobby::Column::Lobby,
                            )
                            .to(MatchmakingLobbies, matchmaking_lobbies::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .col(ColumnDef::new(matchmaking_player_lobby::Column::Character).uuid())
                    .foreign_key(
                        ForeignKeyCreateStatement::new()
                            .from(
                                MatchmakingPlayerLobby,
                                matchmaking_player_lobby::Column::Character,
                            )
                            .to(GameCharacter, game_character::Column::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_lobby::Column::CharacterOther)
                            .string_len(80),
                    )
                    .col(
                        ColumnDef::new(matchmaking_player_lobby::Column::JoinedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        // let stmt = Statement::from_string(manager.get_database_backend(), "COMMIT".to_owned());
        // manager.get_connection().execute(stmt).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Replace the sample below with your own migration scripts

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MatchmakingPlayerLobby)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MatchmakingPlayerInvitation)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MatchmakingLobbies)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().if_exists().table(State).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().if_exists().table(GameCharacter).to_owned())
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MatchmakingSettings)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                Table::drop()
                    .if_exists()
                    .table(MatchmakingInvitation)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().if_exists().table(Users).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().if_exists().table(Game).to_owned())
            .await?;

        manager
            .drop_type(
                Type::drop()
                    .name(LobbyType::LobbyPrivacy)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
