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

use bot::entity::sea_orm::{ConnectOptions, Database, DatabaseConnection};
#[cfg(feature = "migrator")]
use sea_orm_migration::prelude::*;
use config::Config;
use error::RunbackError;
use futures::{
    future::select,
    stream::{FuturesUnordered, StreamExt},
};
use std::{process::exit, sync::Arc};
use tokio::signal::unix::{signal, SignalKind};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Intents};
use twilight_standby::Standby;

use twilight_model::gateway::event::Event;

use crate::interactions::InteractionProcessor;

mod client;
mod config;
mod error;
mod interactions;

// DO NOT STORE THE CONFIG FOR LONG PERIODS OF TIME! IT CAN BE CHANGED ON A WHIM (in the future)
lazy_static! {
    static ref CONFIG: Arc<Box<Config>> = Arc::new(Box::new(Config::new().unwrap()));
}

fn main() {
    let formatter = tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_max_level(Into::<tracing::Level>::into(CONFIG.as_ref().log_level));

    let res = if CONFIG.log_as_json {
        formatter.json().try_init().map_err(|e| anyhow!(e))
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
    let db = connect_to_database()
        .await
        .map_err(|e| anyhow!("Could not connect to database: {}", e))?;
    info!("Successfully connected to database.");

    #[cfg(feature = "migrator")]
    bot::migration::Migrator::up(db.as_ref(), None).await?;

    let cache = Arc::new(
        InMemoryCache::builder()
            .resource_types(ResourceType::MESSAGE)
            .resource_types(ResourceType::CHANNEL)
            .resource_types(ResourceType::MEMBER)
            .resource_types(ResourceType::USER)
            .build(),
    );

    let standby = Arc::new(Standby::new());

    let interactions = Arc::new(
        InteractionProcessor::init(db.clone(), cache.clone(), standby.clone())
            .await
            .map_err(|e| -> anyhow::Error {
                anyhow!("Could not create interaction command handler: {}", e)
            })?,
    ); // Register guild commands
    info!("Registered guild commands");

    // This is the default scheme. It will automatically create as many
    // shards as is suggested by Discord.
    // let scheme = ShardScheme::Bucket { bucket_id: (), concurrency: (), total: () };

    // Use intents to only receive guild message events.
    let (cluster, mut events) = Cluster::builder(CONFIG.token.clone(), Intents::GUILD_MESSAGES)
        .build()
        .await?;
    let cluster = Arc::new(cluster);

    // Start up the cluster.
    let cluster_spawn = Arc::clone(&cluster);

    // Start all shards in the cluster in the background.
    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    let mut executing_futures = FuturesUnordered::new();

    let mut sighup = signal(SignalKind::hangup())?;
    let mut sigint = signal(SignalKind::interrupt())?;

    loop {
        let (s1, s2) = (sighup.recv(), sigint.recv());
        pin!(s1, s2);
        let shutdown = select(s1, s2);

        trace!("running main loop");

        select! {
            Some((shard_id, event)) = events.next() => {
                let cluster_ref = cluster.clone();

                // Update the cache with the event.
                cache.update(&event);
                standby.process(&event);

                trace!(ev = %format!("{:?}", event), "Received Discord event");

                let _shard = match cluster_ref.shard(shard_id) {
                    Some(s) => s,
                    None => {
                        error!(shard = %shard_id, "Invalid shard received during event");
                        // Do some error handling here.
                        continue;
                    }
                };

                match event {
                    Event::Ready(_) => {
                        // Do some intital checks
                        // Check to see if all of the panels related to this shard are healthy
                        info!("Bot is ready!")
                    }
                    Event::InteractionCreate(i) => {
                        let interaction_ref = interactions.clone();
                        let shard = cluster_ref.shard(shard_id).unwrap();
                        let res = interaction_ref.handle_interaction(i, shard);
                        match res {
                            Ok(fut) => {
                                executing_futures.push(fut);
                            },
                            Err(e) => {
                                error!(error = %e, "error occurred while handling interactions.");
                                debug!(debug_error = %format!("{:?}", e), "error occurred while handling interactions.");
                            },
                        }
                    }
                    Event::GatewayHeartbeatAck => {
                        trace!("gateway acked heartbeat");
                    }
                    _ => debug!(kind = %format!("{:?}", event.kind()), "unhandled event"),
                }
            }
            Some(result) = executing_futures.next() => {
                if let Err(e) = result {
                    error!(error = ?e, "Application handler errored out");
                }
            }
            _ = shutdown => {
                info!("received shutdown signal");
                break;
            }
        }
    }

    cluster.clone().down();

    if let Some(guild) = CONFIG.debug_guild_id {
        let client = twilight_http::Client::new(CONFIG.token.clone());

        let application_id = client
            .current_user_application()
            .exec()
            .await?
            .model()
            .await?
            .id;

        let guild_commands = client
            .interaction(application_id)
            .guild_commands(guild)
            .exec()
            .await?
            .model()
            .await?;

        for c in guild_commands {
            // Delete any guild-specific commands
            client
                .interaction(application_id)
                .delete_guild_command(guild, c.id.ok_or_else(|| anyhow!("command has no id"))?)
                .exec()
                .await?;
        }
    }

    Ok(())
}

#[tracing::instrument]
async fn connect_to_database() -> Result<Arc<Box<DatabaseConnection>>, RunbackError> {
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

    let opt = ConnectOptions::new(connection_string);

    // Arc<Box> is easier than setting up a static lifetime reference for the DatabaseConnection
    let db = Arc::new(Box::new(Database::connect(opt).await?));

    Ok(db)
}
