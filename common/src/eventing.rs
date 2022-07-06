use std::{
    any::{self, Any},
    pin::Pin,
    sync::Arc,
    time::Duration,
};

use dashmap::DashMap;
use futures::{Future, FutureExt};
use kafka::{
    consumer::Consumer,
    producer::{Producer, Record},
};
use sea_orm::prelude::Uuid;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
};

macro_rules! type_name {
    ($type:ty) => {
        ::std::any::type_name::<$type>().split("::").last().unwrap()
    };
}

pub struct Orchestrator<'a> {
    message_producer: Producer,
    message_consumer: Consumer,
    outbound_messages: Mutex<Vec<Record<'a, &'a [u8], &'a [u8]>>>,
    // Only one event handler per event type
    /// A map of the Kafka Event Topic -> Event Handler
    event_handlers: RwLock<DashMap<&'static str, Arc<Box<dyn ErasedEventHandler + Send + Sync>>>>,
    futures: Vec<Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>>>,
}

impl<'a> Orchestrator<'a> {
    pub fn new(hosts: Vec<String>) -> anyhow::Result<Self> {
        if hosts.len() == 0 {
            return Err(anyhow!(
                "KafkaClient must receive a non-zero list of hosts to listen on."
            ));
        }

        let id = Uuid::new_v4().to_string();

        let message_producer = Producer::from_hosts(hosts.to_owned())
            .with_client_id(id.to_owned())
            .with_ack_timeout(Duration::from_secs(1))
            .create()?;

        let message_consumer = Consumer::from_hosts(hosts.to_owned())
            .with_client_id(id.to_owned())
            .with_topic("TestEvent".to_string())
            .create()?;

        Ok(Self {
            message_producer,
            outbound_messages: Mutex::new(Vec::with_capacity(128)),
            message_consumer,
            event_handlers: RwLock::new(DashMap::new()),
            futures: Vec::with_capacity(128),
        })
    }

    /// Publish an event through Kafka
    pub async fn publish<T: Event>(&self, event: &T) -> anyhow::Result<()> {
        let mut messages = self.outbound_messages.lock().await;

        let ser = event.serialize();

        if let Err(e) = ser {
            return Err(e);
        }

        messages.push(Record::<'a>::from_key_value(
            type_name!(T),
            Box::leak(Box::new(Uuid::new_v4().as_bytes().to_owned())),
            // SAFETY: I already checked that `ser` is Some() before this call
            Box::leak(unsafe { ser.unwrap_unchecked() }),
        ));

        Ok(())
    }

    /// Add an event handler to the orchestrator
    pub async fn add_event_hander<T>(&self, mut handler: Box<T>) -> anyhow::Result<()>
    where
        T: EventHandler + Send + Sync + 'static,
        T::Event: Send + Sync,
    {
        handler.register()?;

        let _old = self
            .event_handlers
            .write()
            .await
            .insert(type_name!(T::Event), Arc::new(handler));

        Ok(())
    }

    /// Waits for an event given a certain criteria
    pub async fn wait_for_event<T: Event, F: Fn() -> ()>(&self, _f: F) -> T {
        todo!()
    }

    /// Fetch any incoming events and dispatch any event handlers.
    pub async fn process(&mut self) -> anyhow::Result<()> {
        let mut messages = self.outbound_messages.lock().await;

        let _res = self.message_producer.send_all(messages.as_slice())?;

        messages.clear();

        drop(messages);

        let message_sets = self.message_consumer.poll()?;

        for set in message_sets.iter() {
            let topic = set.topic();
            let event_handlers = self.event_handlers.read().await;
            let handler = match event_handlers.get(topic) {
                Some(handler) => handler,
                None => {
                    debug!("No handler found for topic \"{}\"", topic);
                    continue;
                }
            };

            for message in set.messages() {
                let handler = handler.clone();
                let msg = message.value.to_owned();
                let fut = async move { handler.execute(msg.into_boxed_slice()).await };

                self.futures.push(Box::pin(fut));
            }
        }
        
        // TODO: Advance futures, don't just await them all.
        let results = futures::future::join_all(self.futures.as_mut_slice()).await;
        self.futures.clear();

        for res in results {
            if let Err(e) = res {
                error!(error = ?e, "Error encountered while executing event handler");
            }
        }


        Ok(())
    }
}

// impl<'a> Drop for Orchestrator<'a> {
//     fn drop(&mut self) {
//         if self.futures.len() > 0 {
//             let rt = tokio::runtime::Handle::current();

//             let futs = futures::future::join_all(self.futures.as_mut_slice());

//             let results = rt.block_on(futs);

//             for res in results.into_iter().filter_map(|r| r.err()) {
//                 error!( error = ?res, "Encountered error while executing remaining handlers during Orhcestrator drop");
//             }

//             self.futures.clear();
//         }
//     }
// }

/// A struct that holds data about event data
pub struct EventMetadata<T> {
    id: Uuid,
    data: T,
}

pub trait Event: EventSerializer<Self>
where
    Self: Sized,
{
}

#[async_trait]
pub trait EventHandler {
    type Event: Event;

    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    async fn execute(&self, event: Box<Self::Event>) -> anyhow::Result<()>;
}

pub trait EventSerializer<T> {
    fn serialize(&self) -> anyhow::Result<Box<[u8]>>;

    fn deserialize(data: Box<[u8]>) -> anyhow::Result<T>;
}

impl<T> EventSerializer<T> for T
where
    T: Deserialize<'static> + Serialize,
{
    fn serialize(&self) -> anyhow::Result<Box<[u8]>> {
        match serde_json::ser::to_vec(self) {
            Ok(ser) => Ok(ser.into_boxed_slice()),
            Err(e) => {
                error!("Could not serialize {} data: {}", type_name!(T), e);
                Err(anyhow!(e))
            }
        }
    }

    fn deserialize(data: Box<[u8]>) -> anyhow::Result<T> {
        match serde_json::de::from_slice(Box::leak(data)) {
            Ok(deser) => Ok(deser),
            Err(e) => {
                error!("Could not deserialize {} data: {}", type_name!(T), e);
                Err(anyhow!(e))
            }
        }
    }
}

#[async_trait]
pub(crate) trait ErasedEventHandler: Any {
    /// If the EventHandler fails to register, the event handler will not be added.
    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    async fn execute(&self, data: Box<[u8]>) -> anyhow::Result<()>;
}

#[async_trait]
impl<H> ErasedEventHandler for H
where
    H: EventHandler + Send + Sync + 'static,
    H::Event: Send + Sync,
{
    fn register(&mut self) -> anyhow::Result<()> {
        trace!(action = ?"register", handler = ?type_name!(H));
        self.register()
    }

    fn unregister(&mut self) -> anyhow::Result<()> {
        trace!(action = ?"unregister", handler = ?type_name!(H));
        self.unregister()
    }

    async fn execute(&self, data: Box<[u8]>) -> anyhow::Result<()> {
        // Used for debugging only.
        let data_string = String::from_utf8_lossy(data.as_ref()).to_string();
        trace!(
            handler = ?type_name!(H),
            data = ?data_string
        );

        let deser = <<H as EventHandler>::Event as EventSerializer<H::Event>>::deserialize(data);

        match deser {
            Err(e) => {
                error!(data = ?data_string, event = ?type_name!(<H as EventHandler>::Event), error = ?e,
                    "Could not deserialize incoming data for event"
                );
                return Err(anyhow!(
                    "Could not deserialize incoming data for event {}: {}",
                    type_name!(<H as EventHandler>::Event),
                    e
                ));
            }
            Ok(val) => {
                self.execute(Box::new(val)).await?;

                return Ok(());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ops::AddAssign,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use serde::Serialize;

    use super::*;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct TestEvent {
        pub val: u32,
    }

    impl Event for TestEvent {}

    struct TestHandler {
        inner: Arc<Mutex<u32>>,
        registered: Arc<AtomicBool>,
    }

    impl TestHandler {
        pub fn new() -> Self {
            Self {
                inner: Arc::new(Mutex::new(0)),
                registered: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[async_trait]
    impl EventHandler for TestHandler {
        type Event = TestEvent;

        fn register(&mut self) -> anyhow::Result<()> {
            self.registered
                .store(true, std::sync::atomic::Ordering::SeqCst);

            Ok(())
        }

        fn unregister(&mut self) -> anyhow::Result<()> {
            self.registered
                .store(false, std::sync::atomic::Ordering::SeqCst);

            Ok(())
        }

        async fn execute(&self, event: Box<Self::Event>) -> anyhow::Result<()> {
            self.inner.lock().await.add_assign(event.val);

            Ok(())
        }
    }

    #[tokio::test(worker_threads = 4, flavor = "multi_thread")]
    async fn test_orchestrator_fake_event_handler() {
        let mut orchestrator = Orchestrator::new(vec!["kafka:9092".to_string()]).unwrap();

        let handler = TestHandler::new();
        let inner = handler.inner.clone();
        let registered = handler.registered.clone();

        assert!(registered.load(Ordering::Relaxed) == false);

        orchestrator
            .add_event_hander(Box::new(handler))
            .await
            .unwrap();

        orchestrator.publish(&TestEvent { val: 1 }).await.unwrap();
        orchestrator.publish(&TestEvent { val: 2 }).await.unwrap();
        orchestrator.publish(&TestEvent { val: 3 }).await.unwrap();

        assert!(registered.load(Ordering::Relaxed) == true);
        assert!(inner.lock().await.eq(&0));

        orchestrator.process().await.unwrap();

        assert!(inner.lock().await.eq(&6));
    }
}
