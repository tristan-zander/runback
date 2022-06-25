#[macro_use]
extern crate serde;
#[macro_use]
extern crate anyhow;

pub mod auth;
pub mod config;
pub mod logging;
pub mod eventing;

// TODO: Write common logger setup
