use std::{marker::PhantomData, num::NonZeroU64};

use sea_orm::{
    sea_query::{Nullable, ValueType, ValueTypeErr},
    DbErr, TryGetable,
};
use serde::{Deserialize, Serialize};
use twilight_model::id::Id;

pub mod prelude;

pub mod game;
pub mod game_character;
pub mod matchmaking_invitation;
pub mod matchmaking_lobbies;
pub mod matchmaking_player_invitation;
pub mod matchmaking_player_lobby;
pub mod matchmaking_settings;
pub mod sea_orm_active_enums;
pub mod state;
pub mod users;

pub use sea_orm;

#[derive(Debug, Serialize, Deserialize)]
pub struct IdWrapper<T> {
    /// This field is translated into i64 through a memory transmute
    pub inner: NonZeroU64,
    #[serde(skip)]
    data: PhantomData<T>,
}

impl<T> Clone for IdWrapper<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner,
            data: PhantomData {},
        }
    }
}

impl<T> std::fmt::Display for IdWrapper<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl<T> PartialEq for IdWrapper<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> IdWrapper<T> {
    /// Translate the `IdWrapper` into a proper Id.
    #[must_use] pub fn into_id(&self) -> Id<T> {
        Id::from(self.inner)
    }

    #[must_use] pub fn new(n: u64) -> Option<Self> {
        if n == 0 {
            return None;
        }
        unsafe { Some(Self::new_unchecked(n)) }
    }

    /// Create an Id without checking that it could equal 0
    #[must_use] pub unsafe fn new_unchecked(n: u64) -> Self {
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
        let val: Option<i64> = res.try_get(pre, col).map_err(sea_orm::TryGetError::DbErr)?;
        if val.is_none() {
            return Err(sea_orm::TryGetError::Null("Value was null.".to_string()));
        }
        unsafe {
            IdWrapper::from_database_i64(val.unwrap_unchecked()).ok_or(sea_orm::TryGetError::Null(
                "Could not convert i64 into Nonzero u64".to_string(),
            ))
        }
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

        Err(sea_orm::sea_query::ValueTypeErr)
    }

    fn type_name() -> String {
        stringify!(IdWrapper).to_owned()
    }

    fn column_type() -> sea_orm::sea_query::ColumnType {
        i64::column_type()
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
