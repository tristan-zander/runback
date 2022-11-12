use crate::entity::prelude::*;
use sea_orm_migration::prelude::*;

use super::deprecated::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(matchmaking_player_invitation::Entity)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(MatchmakingInvitation)
                    .add_column_if_not_exists(
                        ColumnDef::new(matchmaking_invitation::Column::Lobby).uuid(),
                    )
                    .add_foreign_key(
                        ForeignKey::create()
                            .from(MatchmakingInvitation, matchmaking_invitation::Column::Lobby)
                            .to(MatchmakingLobbies, matchmaking_lobbies::Column::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade)
                            .to_owned()
                            .get_foreign_key(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(matchmaking_invitation::Column::ExtendedTo)
                            .uuid()
                            .not_null(),
                    )
                    .add_foreign_key(
                        ForeignKey::create()
                            .from(
                                MatchmakingInvitation,
                                matchmaking_invitation::Column::ExtendedTo,
                            )
                            .to(Users, users::Column::UserId)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade)
                            .to_owned()
                            .get_foreign_key(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(matchmaking_invitation::Column::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .add_column_if_not_exists(
                        ColumnDef::new(matchmaking_invitation::Column::ChannelId)
                            .big_integer()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Recreate the old table
        manager
            .create_table(
                Table::create()
                    .table(matchmaking_player_invitation::Entity)
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
            .alter_table(
                Table::alter()
                    .table(MatchmakingInvitation)
                    .drop_column(matchmaking_invitation::Column::Lobby)
                    .drop_column(matchmaking_invitation::Column::ExtendedTo)
                    .drop_column(matchmaking_invitation::Column::ExpiresAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
