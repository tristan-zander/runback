use cqrs_es::View;
use cqrs_es::{persist::PersistedEventStore, Aggregate};
use postgres_es::PostgresEventRepository;
use postgres_es::PostgresViewRepository;
use sea_orm::{Database, DatabaseConnection};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

/// This struct facilitates the interaction with the core database,
/// ORM, and event sourcing mechanisms.
/// All database interactions should stem from this struct.
pub struct RunbackDB {
    pool: Pool<Postgres>,
    database_connection: DatabaseConnection,
}

impl RunbackDB {
    pub async fn new(connection_string: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(connection_string)
            .await
            .map_err(|e| anyhow!(e))?;

        let database_connection = Database::connect(connection_string)
            .await
            .map_err(|e| anyhow!(e))?;

        Ok(Self {
            pool,
            database_connection,
        })
    }

    pub fn get_event_store<T: Aggregate>(&self) -> PersistedEventStore<PostgresEventRepository, T> {
        let event_repo = PostgresEventRepository::new(self.pool.clone());
        PersistedEventStore::new_event_store(event_repo)
    }

    pub fn get_view_repository<T: View<A>, A: Aggregate>(&self) -> PostgresViewRepository<T, A> {
        PostgresViewRepository::new("TODO", self.pool.clone())
    }

    pub fn db(&self) -> &DatabaseConnection {
        &self.database_connection
    }
}
