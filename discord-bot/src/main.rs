#[macro_use]
extern crate tracing;

use config::Config;
use futures::stream::StreamExt;
use std::{error::Error, sync::Arc};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{cluster::ShardScheme, Cluster, Intents};
use twilight_http::Client as HttpClient;
use twilight_model::gateway::event::Event;

mod config;
mod interactions;
mod client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_level(true)
        .with_target(true)
        .with_max_level(tracing::Level::DEBUG)
        //.json()
        .init();

    // tracing_log::LogTracer::init()?;

    let config = Config::new()?;

    // Register guild commands
    let command_map = interactions::application_commands::register_all_application_commands(config.clone()).await?;

    info!("Registered guild commands, starting cluster.");

    // This is the default scheme. It will automatically create as many
    // shards as is suggested by Discord.
    let scheme = ShardScheme::Auto;

    // Use intents to only receive guild message events.
    let (cluster, mut events) = Cluster::builder(config.token.to_owned(), Intents::GUILD_MESSAGES)
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
        // Update the cache with the event.
        cache.update(&event);

        debug!(ev = %format!("{:?}", event), "Received Discord event");

        match event {
            Event::Ready(_) => info!("Bot is ready!"),
            Event::InteractionCreate(interaction) => {
                tokio::spawn(interactions::handle_interaction(interaction));
            }
            _ => trace!(kind = %format!("{:?}", event.kind()), "Unhandled event"),
        }
    }

    Ok(())
}
