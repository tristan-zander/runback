#![warn(clippy::pedantic)]

pub mod entity;
#[cfg(feature = "migrator")]
pub mod migration;

#[macro_use]
extern crate tracing;

#[macro_use]
extern crate async_trait;

#[macro_use]
extern crate anyhow;

pub mod events;
pub mod queries;
pub mod services;
