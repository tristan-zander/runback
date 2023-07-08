// Create a discord client for each shard, register handlers and shared state.

use std::mem::MaybeUninit;

use std::sync::Arc;

use crate::{db::RunbackDB, entity::prelude::*};
use futures::future::join;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use sqlx::types::Uuid;
use tokio::signal::unix::{signal, SignalKind};
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{Cluster, Event, Intents};
use twilight_http::client::{ClientBuilder, InteractionClient};
use twilight_model::application::interaction::Interaction;
use twilight_model::channel::Message;
use twilight_model::channel::message::MessageFlags;
use twilight_model::guild::Guild;
use twilight_model::http::interaction::InteractionResponse;
use twilight_model::id::marker::{GuildMarker, UserMarker};
use twilight_model::user::User;
use twilight_model::{
    gateway::payload::incoming::{ChannelDelete, RoleDelete},
    id::{marker::ApplicationMarker, Id},
    user::CurrentUser,
};
use twilight_standby::Standby;
use twilight_util::builder::embed::{EmbedBuilder, EmbedFieldBuilder, EmbedFooterBuilder};

use crate::interactions::InteractionProcessor;

pub struct RunbackClient {
    db: RunbackDB,
    // do NOT use this field in any code that could be called during `new()`.
    // This should only be accessed well after the client has been initialized.
    // This field should never be externally available for this reason.
    interactions: MaybeUninit<InteractionProcessor>,
    discord_client: DiscordClient,
    pub standby: Arc<Standby>,
}

impl RunbackClient {
    pub async fn new(token: String, db: RunbackDB) -> anyhow::Result<Self> {
        let standby = Arc::new(Standby::new());
        let discord_client = DiscordClient::new(token).await?;

        let mut interactions = InteractionProcessor::new(discord_client.clone());
        let mut client = Self {
            standby,
            db,
            interactions: MaybeUninit::uninit(),
            discord_client,
        };

        interactions.init(&client).await?;

        client.interactions = MaybeUninit::new(interactions);

        Ok(client)
    }

    pub async fn run(&self, token: String) -> anyhow::Result<()> {
        // Use intents to only receive guild message events.
        let (cluster, mut events) = Cluster::builder(token, Intents::GUILDS).build().await?;
        let cluster = Arc::new(cluster);

        // Start up the cluster.
        let cluster_spawn = Arc::clone(&cluster);

        // Start all shards in the cluster in the background.
        tokio::spawn(async move {
            cluster_spawn.up().await;
        });

        let mut sighup = signal(SignalKind::hangup())?;
        let mut sigint = signal(SignalKind::interrupt())?;

        let mut executing_futures = FuturesUnordered::new();

        loop {
            trace!("running main loop");

            let shutdown = join(sighup.recv(), sigint.recv());

            select! {
                Some((shard_id, event)) = events.next() => {
                    let cluster_ref = cluster.clone();

                    // Update the cache with the event.
                    self.discord_client.cache.update(&event);
                    self.standby.process(&event);

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
                            // SAFETY: interactions is guaranteed to be there after its constructor.
                            let interaction_ref = unsafe { self.interactions.assume_init_ref() };
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
                        Event::ChannelDelete(chan_delete) => {
                            executing_futures.push(Box::pin(Self::process_channel_delete(self.db(), chan_delete)));
                        }
                        Event::RoleDelete(role_delete) => {
                            executing_futures.push(Box::pin(Self::process_role_delete(self.db(), role_delete)));
                        }
                        Event::GatewayHeartbeatAck => {
                            trace!("gateway acked heartbeat");
                        }
                        _ => debug!(kind = %format!("{:?}", event.kind()), "unhandled event"),
                    }
                }
                Some(result) = executing_futures.next() => {
                    if let Err(e) = result {
                        error!(error = ?e, "application handler error");
                    }
                }
                _ = shutdown => {
                    info!("received shutdown signal");
                    break;
                }
            }
        }

        cluster.clone().down();

        if let Some(guild) = crate::CONFIG.debug_guild_id {
            let client = twilight_http::Client::new(crate::CONFIG.token.clone());

            let application_id = client.current_user_application().await?.model().await?.id;

            let guild_commands = client
                .interaction(application_id)
                .guild_commands(guild)
                .await?
                .model()
                .await?;

            for c in guild_commands {
                // Delete any guild-specific commands
                client
                    .interaction(application_id)
                    .delete_guild_command(guild, c.id.ok_or_else(|| anyhow!("command has no id"))?)
                    .await?;
            }
        }

        Ok(())
    }

    pub fn db(&self) -> RunbackDB {
        self.db.clone()
    }

    pub fn discord(&self) -> DiscordClient {
        self.discord_client.clone()
    }

    #[instrument(skip_all)]
    async fn process_channel_delete(
        db: RunbackDB,
        chan_delete: Box<ChannelDelete>,
    ) -> anyhow::Result<()> {
        let chan_id = chan_delete.id;
        let guild_id = if let Some(gid) = chan_delete.guild_id {
            gid
        } else {
            warn!(channel = ?chan_id, "received channel delete event without receiving the corresponding guild.");
            return Err(anyhow!(""));
        };

        let settings = MatchmakingSettings::find()
            .filter(matchmaking_settings::Column::GuildId.eq(IdWrapper::from(guild_id)))
            .one(db.connection())
            .await?;

        if let Some(settings) = settings {
            if settings
                .channel_id
                .and_then(|v| {
                    if v.into_id() == chan_id {
                        Some(v)
                    } else {
                        None
                    }
                })
                .is_none()
            {
                debug!("channel was not the default matchmaking channel");
                return Ok(());
            }

            MatchmakingSettings::update(matchmaking_settings::ActiveModel {
                guild_id: Set(settings.guild_id),
                channel_id: Set(None),
                ..Default::default()
            })
            .exec(db.connection())
            .await?;

            // TODO: Notify the guild owner that they need to set a new matchmaking channel.

            info!(channel = ?chan_id, guild = ?guild_id, "removed default matchmaking channel because it was deleted");
        } else {
            debug!("not a registered guild");
        }

        Ok(())
    }

    #[instrument(skip_all)]
    async fn process_role_delete(db: RunbackDB, role_delete: RoleDelete) -> anyhow::Result<()> {
        let role_id = role_delete.role_id;
        let guild_id = role_delete.guild_id;

        let settings = MatchmakingSettings::find()
            .filter(matchmaking_settings::Column::GuildId.eq(IdWrapper::from(guild_id)))
            .one(db.connection())
            .await?;

        if let Some(settings) = settings {
            if settings
                .admin_role
                .and_then(|v| {
                    if v.into_id() == role_id {
                        Some(v)
                    } else {
                        None
                    }
                })
                .is_none()
            {
                debug!("role was not the admin role");
                return Ok(());
            }

            MatchmakingSettings::update(matchmaking_settings::ActiveModel {
                guild_id: Set(settings.guild_id),
                channel_id: Set(None),
                ..Default::default()
            })
            .exec(db.connection())
            .await?;

            // TODO: Notify the guild owner that they need to set a new admin role.

            info!(role = ?role_id, guild = ?guild_id, "removed admin role because it was deleted");
        } else {
            debug!("not a registered guild");
        }

        Ok(())
    }
}

/// Provides common interactivity with Discord's HTTP API
#[derive(Debug, Clone)]
pub struct DiscordClient {
    inner: Arc<twilight_http::Client>,
    pub application_id: Id<ApplicationMarker>,
    pub current_user: CurrentUser,
    pub cache: Arc<InMemoryCache>,
}

impl std::ops::Deref for DiscordClient {
    type Target = twilight_http::Client;

    fn deref(&self) -> &Self::Target {
        self.inner.as_ref()
    }
}

impl DiscordClient {
    pub async fn new(token: String) -> anyhow::Result<Self> {
        let inner = Arc::new(ClientBuilder::new().token(token).build());

        let application_id = {
            let response = inner.current_user_application().await?;
            response.model().await?.id
        };

        let current_user = inner.current_user().await?.model().await?;

        Ok(Self {
            inner,
            application_id,
            current_user,
            cache: Arc::new(
                InMemoryCache::builder()
                    .resource_types(ResourceType::MESSAGE)
                    .resource_types(ResourceType::CHANNEL)
                    .resource_types(ResourceType::MEMBER)
                    .resource_types(ResourceType::USER)
                    .build(),
            ),
        })
    }

    pub fn interaction(&self) -> InteractionClient<'_> {
        self.inner.interaction(self.application_id)
    }

    pub async fn send_message(
        &self,
        interaction: &Interaction,
        message: &InteractionResponse,
    ) -> anyhow::Result<()> {
        let res = self
            .interaction()
            .create_response(interaction.id, interaction.token.as_str(), message)
            .await?;

        debug!("Send Message response: {:#?}", res);

        Ok(())
    }

    pub async fn get_user(&self, user: Id<UserMarker>) -> anyhow::Result<User> {
        if let Some(user_ref) = self.cache.user(user) {
            let user = user_ref.to_owned();
            return Ok(user);
        }

        let user = self.user(user).await?.model().await?;
        Ok(user)
    }

    pub async fn get_guild(&self, guild_id: Id<GuildMarker>) -> anyhow::Result<Guild> {
        let res = self.guild(guild_id).await?;
        return Ok(res.model().await.map_err(|e| anyhow!(e))?);
    }

    pub async fn respond_to_user(&self) -> anyhow::Result<()> {
        unimplemented!()
    }

    pub async fn ack(&self, interaction_token: &str) -> anyhow::Result<()> {
        self.interaction()
            .delete_response(interaction_token)
            .await?;

        Ok(())
    }

    pub async fn send_error_response(
        &self,
        token: &str,
        error_id: Uuid,
        error_message: &str,
    ) -> anyhow::Result<Message> {
        let res = self
            .interaction()
            .create_followup(token)
            .flags(MessageFlags::EPHEMERAL)
            .embeds(&[EmbedBuilder::new()
                .description("An error has occurred.")
                .footer(EmbedFooterBuilder::new(error_id.hyphenated().to_string()).build())
                .field(EmbedFieldBuilder::new("error", error_message).build())
                .validate()?
                .build()])?
            .await?.model().await?;

        Ok(res)
    }
}
