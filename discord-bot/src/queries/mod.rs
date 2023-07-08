use std::marker::PhantomData;

use cqrs_es::{
    persist::{PersistenceError, ViewContext, ViewRepository},
    Aggregate, View,
};
use sea_orm::{sea_query::Mode, DatabaseConnection, EntityTrait, ModelTrait};

pub mod lobby;

pub struct SeaOrmViewRepository<V, A> {
    connection: DatabaseConnection,
    _phantom: PhantomData<(V, A)>,
}

#[async_trait]
impl<V, A> ViewRepository<V, A> for SeaOrmViewRepository<V, A>
where
    A: Aggregate + ModelTrait,
    V: View<A>,
{
    async fn load(&self, view_id: &str) -> Result<Option<V>, PersistenceError> {
        unimplemented!();
    }

    async fn load_with_context(
        &self,
        view_id: &str,
    ) -> Result<Option<(V, ViewContext)>, PersistenceError> {
        unimplemented!()
    }

    async fn update_view(&self, view: V, context: ViewContext) -> Result<(), PersistenceError> {
        <A::Entity as EntityTrait>::find();
        unimplemented!()
    }
}
