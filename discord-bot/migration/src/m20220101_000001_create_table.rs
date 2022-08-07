use sea_orm_migration::{prelude::*, sea_query::extension::postgres::Type};

use entity::{
    discord_user,
    matchmaking::{self, lobby::LobbyPrivacy},
};

use chrono::Utc;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220101_000001_create_table"
    }
}

#[derive(Iden)]
enum LobbyType {
    LobbyPrivacy,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
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
                sea_query::Table::create()
                    .table(matchmaking::Lobby)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking::lobby::Column::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lobby::Column::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lobby::Column::TimeoutAfter)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(matchmaking::lobby::Column::ThreadId).big_integer())
                    .col(ColumnDef::new(matchmaking::lobby::Column::Description).string_len(80))
                    .col(
                        ColumnDef::new(matchmaking::lobby::Column::Owner)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lobby::Column::Privacy)
                            .enumeration("lobby_privacy", ["Open", "InviteOnly"])
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                sea_query::Table::create()
                    .table(matchmaking::LookingForGames)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking::lfg::Column::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lfg::Column::GuildId)
                            .big_integer()
                            .unique_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lfg::Column::UserId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lfg::Column::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::lfg::Column::TimeoutAfter)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(matchmaking::Panel)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking::panel::Column::PanelId)
                            .uuid()
                            .primary_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::panel::Column::GuildId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::panel::Column::ChannelId)
                            .big_integer()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::panel::Column::MessageId)
                            .big_integer()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(matchmaking::panel::Column::Game).string_len(80))
                    .col(ColumnDef::new(matchmaking::panel::Column::Comment).string_len(255))
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(matchmaking::Setting)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking::settings::Column::GuildId)
                            .big_integer()
                            .primary_key()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::settings::Column::HasAcceptedEula)
                            // Will be null or the date that the admin accepted the EULA
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::settings::Column::LastUpdated)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Utc::now()),
                    )
                    .col(ColumnDef::new(matchmaking::settings::Column::ChannelId).big_integer())
                    .col(
                        ColumnDef::new(matchmaking::settings::Column::ThreadsArePrivate)
                            .boolean()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(discord_user::User)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(discord_user::user::Column::UserId)
                            .big_integer()
                            .primary_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(discord_user::user::Column::ActiveSession).integer())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                sea_query::Table::drop()
                    .table(matchmaking::Lobby)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(matchmaking::LookingForGames).to_owned())
            .await?;

        manager
            .drop_table(
                sea_query::Table::drop()
                    .table(matchmaking::Panel)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                sea_query::Table::drop()
                    .table(matchmaking::Setting)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(
                sea_query::Table::drop()
                    .table(discord_user::User)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_type(
                Type::drop()
                    .if_exists()
                    .name(LobbyType::LobbyPrivacy)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
