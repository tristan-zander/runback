use migration::Migrator;
use sea_orm_migration::*;

#[tokio::main]
async fn main() {
    cli::run_cli(Migrator).await;
}
