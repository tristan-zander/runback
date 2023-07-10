use sea_orm_migration::prelude::*;

mod deprecated;

mod m20221004_222202_create_tables;
mod m20221111_215309_remove_invitation_junction;
mod m20230709_220559_add_eventing;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20221004_222202_create_tables::Migration),
            Box::new(m20221111_215309_remove_invitation_junction::Migration),
            Box::new(m20230709_220559_add_eventing::Migration),
        ]
    }
}
