use sea_schema::migration::{
    sea_query::{self, *},
    *,
};

use entity::{discord_user, matchmaking};

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20220101_000001_create_table"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                sea_query::Table::create()
                    .table(matchmaking::ActiveSession)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(matchmaking::active_session::Column::Id)
                            .integer()
                            .not_null()
                            .primary_key()
                            .auto_increment(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::active_session::Column::StartedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::active_session::Column::TimeoutAfter)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::active_session::Column::ThreadId).big_integer(),
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
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(matchmaking::panel::Column::MessageId)
                            .big_integer()
                            .unique_key()
                            .not_null(),
                    )
                    .col(ColumnDef::new(matchmaking::panel::Column::Game).string_len(80))
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
                            .timestamp_with_time_zone(),
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
                    .table(matchmaking::ActiveSession)
                    .to_owned(),
            )
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

        Ok(())
    }
}
