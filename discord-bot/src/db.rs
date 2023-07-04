use cqrs_es::persist::GenericQuery;
use cqrs_es::{persist::PersistedEventStore, Aggregate};
use postgres_es::PostgresEventRepository;
use postgres_es::PostgresViewRepository;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::events::Lobby;

/// This struct facilitates the interaction with the core database,
/// ORM, and event sourcing mechanisms.
/// All database interactions should stem from this struct.
pub struct RunbackDB {
    pool: Pool<Postgres>,
    event_repo: PostgresEventRepository,
}

impl RunbackDB {
    pub async fn new(connection_string: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(connection_string)
            .await
            .map_err(|e| anyhow!(e))?;
        let event_repo = PostgresEventRepository::new(pool);

        Ok(Self { event_repo, pool })
    }

    pub fn get_event_store<T: Aggregate>(&self) -> PersistedEventStore<PostgresEventRepository, T> {
        PersistedEventStore::new_event_store(self.event_repo)
    }

    pub fn get_view_repository<T: View, A: Aggregate>(&self) -> PostgresViewRepository<T, A> {
        PostgresViewRepository::new("TODO", self.pool)
    }
}
