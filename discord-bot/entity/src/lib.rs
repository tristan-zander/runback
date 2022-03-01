#[macro_use]
extern crate tracing;

use std::{marker::PhantomData, num::NonZeroU64};

pub use sea_orm;
use sea_orm::{
    sea_query::{Nullable, ValueType, ValueTypeErr},
    DbErr, TryGetable,
};
use twilight_model::id::Id;

pub mod discord_user;
pub mod matchmaking;

#[derive(Debug, Clone)]
pub struct IdWrapper<T> {
    /// This field is translated into i64 through a memory transmute
    pub inner: NonZeroU64,
    data: PhantomData<T>,
}

impl<T> PartialEq for IdWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> IdWrapper<T> {
    /// Translate the IdWrapper into a proper Id.
    pub fn into_id(&self) -> Id<T> {
        Id::from(self.inner)
    }

    pub fn new(n: u64) -> Option<Self> {
        if n == 0 {
            return None;
        }
        unsafe { Some(Self::new_unchecked(n)) }
    }

    /// Create an Id without checking that it could equal 0
    pub unsafe fn new_unchecked(n: u64) -> Self {
        IdWrapper {
            inner: NonZeroU64::new_unchecked(n),
            data: PhantomData {},
        }
    }

    fn from_database_i64(inner: i64) -> Option<Self> {
        let inner_as_u64: u64 = unsafe { std::mem::transmute(inner) };
        if inner_as_u64 == 0 {
            return None;
        }

        Some(Self {
            inner: unsafe { NonZeroU64::new_unchecked(inner_as_u64) },
            data: PhantomData {},
        })
    }

    fn to_database_i64(&self) -> i64 {
        unsafe { std::mem::transmute(self.inner) }
    }
}

impl<T> From<IdWrapper<T>> for i64 {
    fn from(id: IdWrapper<T>) -> Self {
        id.to_database_i64()
    }
}

impl<T> sea_orm::TryFromU64 for IdWrapper<T> {
    fn try_from_u64(n: u64) -> Result<Self, sea_orm::DbErr> {
        Self::new(n).ok_or(DbErr::Custom(
            "Could not convert u64 to i64 for usage as an IdWrapper".to_owned(),
        ))
    }
}

impl<T> From<IdWrapper<T>> for sea_orm::Value {
    fn from(id: IdWrapper<T>) -> Self {
        debug!(val = %format!("{:?}", Self::BigInt(Some(id.to_database_i64()))), "From IdWrapper to Value");
        Self::BigInt(Some(id.to_database_i64()))
    }
}

impl<T: std::fmt::Debug> TryGetable for IdWrapper<T> {
    fn try_get(
        res: &sea_orm::QueryResult,
        pre: &str,
        col: &str,
    ) -> Result<Self, sea_orm::TryGetError> {
        let val: i64 = res.try_get(pre, col).map_err(sea_orm::TryGetError::DbErr)?;
        debug!(val = %val, "TryGetable");
        IdWrapper::from_database_i64(val).ok_or(sea_orm::TryGetError::Null)
    }
}

impl<T> Nullable for IdWrapper<T> {
    fn null() -> sea_orm::Value {
        sea_orm::Value::BigInt(None)
    }
}

impl<T> ValueType for IdWrapper<T> {
    fn try_from(v: sea_orm::Value) -> Result<Self, sea_orm::sea_query::ValueTypeErr> {
        debug!(val = %format!("{:?}", v), "Value for ValueType");
        if let sea_orm::Value::BigInt(inner) = v {
            if inner.is_some() {
                unsafe {
                    // We've already asserted that the i64 is some
                    return Self::from_database_i64(inner.unwrap_unchecked()).ok_or(ValueTypeErr);
                }
            }
        }

        return Err(sea_orm::sea_query::ValueTypeErr);
    }

    fn type_name() -> String {
        stringify!(IdWrapper).to_owned()
    }

    fn column_type() -> sea_orm::sea_query::ColumnType {
        sea_orm::sea_query::ColumnType::BigInteger(None)
    }
}

impl<T> From<IdWrapper<T>> for u64 {
    fn from(val: IdWrapper<T>) -> Self {
        u64::from(val.inner)
    }
}

impl<T> From<IdWrapper<T>> for Id<T> {
    fn from(val: IdWrapper<T>) -> Self {
        val.into_id()
    }
}

impl<T> From<u64> for IdWrapper<T> {
    /// Will panic if n == 0
    fn from(n: u64) -> Self {
        Self::new(n).unwrap()
    }
}

impl<T> From<Id<T>> for IdWrapper<T> {
    fn from(id: Id<T>) -> Self {
        // Id's are always nonzero, so this is fine.
        unsafe { Self::new_unchecked(id.get()) }
    }
}
