#[macro_use]
extern crate tracing;
#[macro_use]
extern crate lazy_static;

use config::Config;
use entity::sea_orm::{ConnectOptions, Database};
use futures::stream::StreamExt;
use migration::DbErr;
use std::{error::Error, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{
    cluster::{ClusterStartError, ShardScheme},
    Cluster, Intents,
};

use twilight_model::gateway::event::Event;

use crate::interactions::InteractionHandler;

mod client;
mod config;
mod interactions;

// DO NOT STORE THE CONFIG FOR LONG PERIODS OF TIME! IT CAN BE CHANGED ON A WHIM (in the future)
lazy_static! {
    static ref CONFIG: Arc<Box<Config>> = Arc::new(Box::new(Config::new().unwrap()));
}

#[derive(Debug)]
pub struct RunbackError {
    pub message: String,
    pub inner: Option<Box<dyn Error + 'static>>,
}

impl Error for RunbackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            Some(e) => Some(e.as_ref()),
            None => None,
        }
    }
}

impl std::fmt::Display for RunbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<DbErr> for RunbackError {
    fn from(e: DbErr) -> Self {
        RunbackError {
            message: "Database Error".to_owned(),
            inner: Some(e.into()),
        }
    }
}

impl From<ClusterStartError> for RunbackError {
    fn from(e: ClusterStartError) -> Self {
        RunbackError {
            message: "Cluster Start Error".to_owned(),
            inner: Some(e.into()),
        }
    }
}

impl From<twilight_http::Error> for RunbackError {
    fn from(e: twilight_http::Error) -> Self {
        RunbackError {
            message: "Twilight HTTP Error".to_owned(),
            inner: Some(e.into()),
        }
    }
}

impl From<Box<dyn Error>> for RunbackError {
    fn from(e: Box<dyn Error>) -> Self {
        RunbackError {
            message: "Unknown Error".to_owned(),
            inner: Some(e.into()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), RunbackError> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_max_level(Into::<tracing::Level>::into(CONFIG.as_ref().log_level))
        //.json()
        .init();

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
    info!("Successfully connected to database.");

    // tracing_log::LogTracer::init()?;

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
                    interaction_ref.handle_interaction(i, shard).await?;
                });
            }
            _ => trace!(kind = %format!("{:?}", event.kind()), "Unhandled event"),
        }
    }

    return Ok(());
}
