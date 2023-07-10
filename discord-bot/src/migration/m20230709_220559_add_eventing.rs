use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const EVENTS_SQL: &str = include_str!("./res/event-init.sql");
const SNAPSHOTS_SQL: &str = include_str!("./res/snapshot-init.sql");

#[derive(Iden)]
enum EventTables {
    Events,
    Snapshots,
}

#[async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute(Statement::from_string(
            manager.get_database_backend(),
            EVENTS_SQL.to_string(),
        ))
        .await?;

        db.execute(Statement::from_string(
            manager.get_database_backend(),
            SNAPSHOTS_SQL.to_string(),
        ))
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(EventTables::Snapshots)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(EventTables::Events)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
