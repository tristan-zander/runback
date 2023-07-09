use std::{error::Error, marker::PhantomData, str::FromStr, sync::Arc};

use cqrs_es::{
    persist::{PersistenceError, ViewContext, ViewRepository},
    Aggregate, View,
};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, IntoActiveModel,
    ModelTrait, PrimaryKeyTrait,
};

pub mod lobby;

/// Provides the version column for a SeaORM Model.
pub trait MaterializedViewTrait {
    type VersionColumnType: ColumnTrait;

    fn version_column() -> Self::VersionColumnType;
}

pub struct SeaOrmViewRepository<V, A> 
where
    A: Aggregate + ModelTrait + ActiveModelTrait + MaterializedViewTrait,
    V: View<A> + IntoActiveModel<A>,
    <<<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType as FromStr>::Err:
        Into<Box<dyn Error + Send + Sync + 'static>>,
    <<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: FromStr,
    Option<<<A as ModelTrait>::Entity as sea_orm::EntityTrait>::Model>: Into<Option<V>>,
    <<A as ActiveModelTrait>::Entity as EntityTrait>::Model: IntoActiveModel<A>
 {
    pub connection: Arc<DatabaseConnection>,
    pub phantom: PhantomData<(V, A)>,
}

impl<V, A> SeaOrmViewRepository<V, A>
where
    A: Aggregate + ModelTrait + ActiveModelTrait + MaterializedViewTrait,
    V: View<A> + IntoActiveModel<A>,
    <<<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType as FromStr>::Err:
        Into<Box<dyn Error + Send + Sync + 'static>>,
    <<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: FromStr,
    Option<<<A as ModelTrait>::Entity as sea_orm::EntityTrait>::Model>: Into<Option<V>>,
    <<A as ActiveModelTrait>::Entity as EntityTrait>::Model: IntoActiveModel<A>
{
    pub async fn create_view(&self, view: V, context: ViewContext) -> anyhow::Result<()> {
        let res = <A as ActiveModelTrait>::Entity::insert(view.into_active_model()).exec(self.connection.as_ref()).await?;
        Ok(())
    }

    pub async fn update_view(&self, view: V, context: ViewContext) -> anyhow::Result<()> {
        <A as ActiveModelTrait>::Entity::update(view.into_active_model()).exec(self.connection.as_ref()).await?;
        Ok(())
    }
}

#[async_trait]
impl<V, A> ViewRepository<V, A> for SeaOrmViewRepository<V, A>
where
    A: Aggregate + ModelTrait + ActiveModelTrait + MaterializedViewTrait,
    V: View<A> + IntoActiveModel<A>,
    <<<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType as FromStr>::Err:
        Into<Box<dyn Error + Send + Sync + 'static>>,
    <<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType: FromStr,
    Option<<<A as ModelTrait>::Entity as sea_orm::EntityTrait>::Model>: Into<Option<V>>,
    <<A as ActiveModelTrait>::Entity as EntityTrait>::Model: IntoActiveModel<A>
{
    async fn load(&self, view_id: &str) -> Result<Option<V>, PersistenceError> {
        let key = <<<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType as FromStr>::from_str(view_id).map_err(|e| PersistenceError::UnknownError(e.into()))?;
        let view = <<A as ModelTrait>::Entity as EntityTrait>::find_by_id(key)
            .one(self.connection.as_ref())
            .await
            .map_err(|e| match e {
                DbErr::Conn(conn) => PersistenceError::ConnectionError(conn.into()),
                DbErr::ConnectionAcquire => PersistenceError::ConnectionError(
                    anyhow!("could not acquire a connection from the pool.").into(),
                ),
                _ => PersistenceError::UnknownError(e.into()),
            })?;

        return Ok(view.into());
    }

    async fn load_with_context(
        &self,
        view_id: &str,
    ) -> Result<Option<(V, ViewContext)>, PersistenceError> {
        let key = <<<<A as ModelTrait>::Entity as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType as FromStr>::from_str(view_id).map_err(|e| PersistenceError::UnknownError(e.into()))?;
        let view_res = <<A as ModelTrait>::Entity as EntityTrait>::find_by_id(key)
            .one(self.connection.as_ref())
            .await
            .map_err(|e| match e {
                DbErr::Conn(conn) => PersistenceError::ConnectionError(conn.into()),
                DbErr::ConnectionAcquire => PersistenceError::ConnectionError(
                    anyhow!("could not acquire a connection from the pool.").into(),
                ),
                _ => PersistenceError::UnknownError(e.into()),
            })?.into();

        if let Some(view) = view_res {
            return Ok(Some((view, ViewContext::new(view_id.to_string(), 0))));
        } else {
            return Ok(None);
        }
    }

    async fn update_view(&self, view: V, context: ViewContext) -> Result<(), PersistenceError> {
        let exists = context.version > 0;
        if exists {
            self.update_view(view, context)
                .await
                .map_err(|e| PersistenceError::UnknownError(e.into()))?;
        } else {
            self.create_view(view, context)
                .await
                .map_err(|e| PersistenceError::UnknownError(e.into()))?;
        }
        Ok(())
    }
}
