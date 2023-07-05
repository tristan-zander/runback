use crate::entity::prelude::*;
use chrono::Utc;
use cqrs_es::View;
use cqrs_es::{persist::PersistedEventStore, Aggregate};
use postgres_es::PostgresEventRepository;
use postgres_es::PostgresViewRepository;
use sea_orm::{Database, DatabaseConnection, IntoActiveModel};
use sqlx::postgres::PgPoolOptions;
use sqlx::types::Uuid;
use sqlx::{Pool, Postgres};
use twilight_model::id::marker::{GuildMarker, UserMarker};
use twilight_model::id::Id;

/// This struct facilitates the interaction with the core database,
/// ORM, and event sourcing mechanisms.
/// All database interactions should stem from this struct.
#[derive(Clone)]
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

    pub fn connection(&self) -> &DatabaseConnection {
        &self.database_connection
    }

    #[cfg(feature = "migrator")]
    pub async fn migrate(&self) -> anyhow::Result<()> {
        use sea_orm_migration::MigratorTrait;

        crate::migration::Migrator::up(self.connection(), None).await?;
        Ok(())
    }

    /// If the guild does not exist, it will create the settings with the default settings
    /// and commit it to the database.
    pub async fn get_guild_settings(
        &self,
        guild: Id<GuildMarker>,
    ) -> anyhow::Result<matchmaking_settings::Model> {
        use matchmaking_settings as settings;
        use MatchmakingSettings as Setting;

        let guild_id: IdWrapper<_> = guild.into();
        let setting = Setting::find_by_id(guild_id.clone())
            .one(self.connection())
            .await?;

        match setting {
            Some(setting) => Ok(setting),
            None => {
                let setting = settings::ActiveModel {
                    guild_id: Set(guild_id),
                    last_updated: Set(Utc::now()),
                    ..Default::default()
                };

                let setting = Setting::insert(setting)
                    .exec_with_returning(self.connection())
                    .await?;

                Ok(setting)
            }
        }
    }

    pub async fn find_or_create_user(&self, id: Id<UserMarker>) -> anyhow::Result<users::Model> {
        let res = Users::find()
            .filter(users::Column::DiscordUser.eq(IdWrapper::from(id)))
            .one(self.connection())
            .await?;

        if let Some(user) = res {
            Ok(user)
        } else {
            let user = users::Model {
                user_id: Uuid::new_v4(),
                discord_user: Some(id.into()),
            };

            let user = Users::insert(user.into_active_model())
                .exec_with_returning(self.connection())
                .await?;

            return Ok(user);
        }
    }
}
