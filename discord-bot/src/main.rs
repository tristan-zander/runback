#![warn(clippy::pedantic)]

#[macro_use]
extern crate tracing;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate async_trait;
#[macro_use]
extern crate tokio;

pub mod client;
pub mod config;
pub mod db;
pub mod entity;
pub mod error;
pub mod events;
pub mod interactions;
#[cfg(feature = "migrator")]
pub mod migration;
pub mod queries;
pub mod services;

use crate::{
    db::RunbackDB,
    entity::sea_orm::{ConnectOptions, Database, DatabaseConnection},
};
use config::Config;
use error::RunbackError;
use std::{process::exit, sync::Arc};
use tracing_subscriber::EnvFilter;

use crate::client::RunbackClient;

// DO NOT STORE THE CONFIG FOR LONG PERIODS OF TIME! IT CAN BE CHANGED ON A WHIM (in the future)
lazy_static! {
    static ref CONFIG: Arc<Box<Config>> = Arc::new(Box::new(Config::new().unwrap()));
}

fn main() {
    let filter = EnvFilter::from_default_env()
        .add_directive("twilight_gateway=info".parse().unwrap())
        .add_directive("twilight_gateway_queue=info".parse().unwrap())
        .add_directive("twilight_http_ratelimiting=info".parse().unwrap())
        .add_directive("rustls=info".parse().unwrap())
        .add_directive("h2=info".parse().unwrap())
        .add_directive("hyper=info".parse().unwrap())
        .add_directive("tungstenite=info".parse().unwrap())
        .add_directive(Into::<tracing::Level>::into(CONFIG.as_ref().log_level).into());

    let formatter = tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_line_number(true)
        .with_env_filter(filter);

    let res = if CONFIG.log_as_json {
        formatter.json().try_init().map_err(|e| anyhow!(e))
    } else if cfg!(debug_assertions) {
        formatter.pretty().try_init().map_err(|e| anyhow!(e))
    } else {
        formatter.try_init().map_err(|e| anyhow!(e))
    };

    if let Err(e) = res {
        eprintln!("could not set up logger: {}", e);
        exit(1);
    }

    let res = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(entrypoint());

    if let Err(e) = res {
        error!(error = ?e, "a fatal error has occurred");
        exit(1);
    }
}

async fn entrypoint() -> anyhow::Result<()> {
    let connection_string = create_connection_string();
    let db = RunbackDB::new(connection_string.as_str()).await?;

    #[cfg(feature = "migrator")]
    db.migrate().await?;

    let runback_client = RunbackClient::new(crate::CONFIG.token.clone(), db).await?;

    // This is the default scheme. It will automatically create as many
    // shards as is suggested by Discord.
    // let scheme = ShardScheme::Bucket { bucket_id: (), concurrency: (), total: () };

    runback_client.run(crate::CONFIG.token.clone()).await?;

    Ok(())
}

fn create_connection_string() -> String {
    let db = &CONFIG.db;
    let pass = if db.password.is_some() {
        format!(":{}", db.password.as_ref().unwrap())
    } else {
        "".to_string()
    };

    let connection_string = format!(
        "{}://{}{}@{}:{}/{}{}",
        db.protocol, db.username, pass, db.host, db.port, db.db_name, db.extra_options
    );

    info!(host = ?db.host, "Connecting to database");
    debug!(connection_string = ?connection_string);

    connection_string
}

#[tracing::instrument]
async fn connect_to_database() -> Result<Arc<Box<DatabaseConnection>>, RunbackError> {
    let connection_string = create_connection_string();

    let opt = ConnectOptions::new(connection_string);

    // Arc<Box> is easier than setting up a static lifetime reference for the DatabaseConnection
    let db = Arc::new(Box::new(Database::connect(opt).await?));

    Ok(db)
}
