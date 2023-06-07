#![warn(clippy::pedantic)]

pub mod entity;
#[cfg(feature = "migrator")]
pub mod migration;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate async_trait;

pub mod events;
pub mod services;
pub mod queries;