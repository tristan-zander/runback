#[macro_use]
extern crate tracing;
#[macro_use]
extern crate lazy_static;

use config::Config;
use entity::sea_orm::{ConnectOptions, Database, DatabaseConnection};
use error::RunbackError;
use futures::stream::StreamExt;
use std::sync::Arc;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{cluster::ShardScheme, Cluster, Intents};

use twilight_model::gateway::event::Event;

use crate::interactions::InteractionHandler;

mod client;
mod config;
mod error;
mod interactions;

// DO NOT STORE THE CONFIG FOR LONG PERIODS OF TIME! IT CAN BE CHANGED ON A WHIM (in the future)
lazy_static! {
    static ref CONFIG: Arc<Box<Config>> = Arc::new(Box::new(Config::new().unwrap()));
}

#[tokio::main]
async fn main() -> Result<(), RunbackError> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_max_level(Into::<tracing::Level>::into(CONFIG.as_ref().log_level))
        //.json()
        .init();

    let db = connect_to_database().await?;
    info!("Successfully connected to database.");

    let interactions = Arc::new(InteractionHandler::init(db.clone()).await?); // Register guild commands
    info!("Registered guild commands");

    // This is the default scheme. It will automatically create as many
    // shards as is suggested by Discord.
    let scheme = ShardScheme::Auto;

    // Use intents to only receive guild message events.
    let (cluster, mut events) = Cluster::builder(CONFIG.token.clone(), Intents::GUILD_MESSAGES)
        .shard_scheme(scheme)
        .build()
        .await?;
    let cluster = Arc::new(cluster);

    // Start up the cluster.
    let cluster_spawn = Arc::clone(&cluster);

    // Start all shards in the cluster in the background.
    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    // Since we only care about new messages, make the cache only
    // cache new messages.
    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MESSAGE)
        .build();

    // Process each event as they come in.
    while let Some((shard_id, event)) = events.next().await {
        let cluster_ref = cluster.clone();

        // Update the cache with the event.
        cache.update(&event);

        debug!(ev = %format!("{:?}", event), "Received Discord event");

        let _shard = match cluster_ref.shard(shard_id) {
            Some(s) => s,
            None => {
                error!(shard = %shard_id, "Invalid shard received during event");
                // Do some error handling here.
                continue;
            }
        };

        match event {
            Event::Ready(_) => info!("Bot is ready!"),
            Event::InteractionCreate(i) => {
                let interaction_ref = interactions.clone();
                tokio::spawn(async move {
                    let shard = cluster_ref.shard(shard_id).unwrap();
                    let res = interaction_ref.handle_interaction(i, shard).await;
                    if let Err(e) = res {
                        error!(error = %e, "Error occurred while handling interactions.");
                        debug!(debug_error = %format!("{:?}", e), "Error occurred while handling interactions.");
                    }
                });
            }
            _ => trace!(kind = %format!("{:?}", event.kind()), "Unhandled event"),
        }
    }

    return Ok(());
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
        "{}://{}{}@{}/{}{}",
        db.protocol, db.username, pass, db.host, db.db_name, db.extra_options
    );

    let opt = ConnectOptions::new(connection_string);

    // Arc<Box> is easier than setting up a static lifetime reference for the DatabaseConnection
    let db = Arc::new(Box::new(Database::connect(opt).await?));
    

    Ok(db)
}
