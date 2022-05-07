use std::sync::Arc;

use calloop::{
    futures::{Executor, Scheduler},
    EventSource, LoopHandle, RegistrationToken,
};
use twilight_cache_inmemory::InMemoryCache;
use twilight_gateway::{cluster::Events, Cluster, Intents};

pub struct EventData {
    pub shard_id: Option<u64>,
    pub discord_event: Option<twilight_gateway::Event>,
    pub cache: InMemoryCache,
}

pub struct DiscordGatewayEvents {
    executor: Executor<twilight_gateway::Event>,
    scheduler: Scheduler<twilight_gateway::Event>,
    events: Events,
    cluster: Arc<Cluster>,
}

impl DiscordGatewayEvents {
    pub fn new(cluster: Arc<Cluster>, events: Events) -> anyhow::Result<Self> {
        let (exec, sched) = calloop::futures::executor()?;
        Self::from_executor(exec, sched, cluster, events)
    }

    pub fn from_executor(
        executor: Executor<twilight_gateway::Event>,
        scheduler: Scheduler<twilight_gateway::Event>,
        cluster: Arc<Cluster>,
        events: Events,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            executor,
            scheduler,
            events,
            cluster,
        })
    }

    pub fn insert_into_event_loop(
        self,
        ev: LoopHandle<EventData>,
    ) -> anyhow::Result<RegistrationToken> {
        match ev.insert_source(self.executor, |_, _, _| {}) {
            Ok(token) => Ok(token),
            Err(e) => Err(anyhow::Error::new(e.error)),
        }
    }
}

impl EventSource for DiscordGatewayEvents {
    type Event = twilight_gateway::Event;

    // Maybe attach interaction ids?
    type Metadata = ();

    // Returning a discord message *might* be a good idea.
    type Ret = ();

    type Error = anyhow::Error;

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        callback: F,
    ) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.executor.process_events(readiness, token, callback).map_err(|e| anyhow::Error::new(e))
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.executor.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.executor.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.executor.unregister(poll)
    }
}
