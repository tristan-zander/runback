use std::{
    any::{Any, TypeId},
    sync::Arc,
    time::Duration,
};

use dashmap::DashMap;
use futures::{future::BoxFuture, stream::FuturesUnordered, FutureExt, StreamExt};
use kafka::{
    consumer::Consumer,
    producer::{Producer, Record},
};
use sea_orm::prelude::Uuid;
use serde::{Deserialize, Serialize};
use tokio::sync::{
    mpsc::{error::SendError, Receiver, Sender},
    Mutex, RwLock,
};

macro_rules! type_name {
    ($type:ty) => {
        ::std::any::type_name::<$type>().split("::").last().unwrap()
    };
}

type RecordType<'a> = Record<'a, &'a [u8], &'a [u8]>;

#[derive(Error, Debug)]
pub enum EventingError {
    #[error(
        "KafkaClient must receieve a non-zero list of hosts to listen on. Actual length: {0}."
    )]
    NoHostSpecified(usize),
    #[error("Could not publish event to message bus.")]
    CouldNotPubilsh,
}

const MESSAGE_BUF_LEN: usize = 128;

pub struct EventOrchestrator<'a> {
    // TODO: Make this a raw KafkaClient eventually
    kafka_producer: Arc<Mutex<Producer>>,
    kafka_consumer: Arc<Mutex<Consumer>>,
    /// A map of the Kafka Event Topic -> Event Handler
    event_handlers: RwLock<DashMap<&'static str, Arc<Box<dyn ErasedEventHandler + Send + Sync>>>>,
    futures: Arc<Mutex<FuturesUnordered<BoxFuture<'a, anyhow::Result<()>>>>>,
    event_sender: Sender<RecordType<'a>>,
    event_receiver: Receiver<RecordType<'a>>,
}

impl<'a> EventOrchestrator<'a> {
    pub fn new(hosts: Vec<String>) -> anyhow::Result<Self> {
        if hosts.len() == 0 {
            return Err(anyhow!(EventingError::NoHostSpecified(hosts.len())));
        }

        let id = Uuid::new_v4().to_string();

        let message_producer = Arc::new(Mutex::new(
            Producer::from_hosts(hosts.to_owned())
                .with_client_id(id.to_owned())
                .with_ack_timeout(Duration::from_secs(1))
                .create()?,
        ));

        let message_consumer = Arc::new(Mutex::new(
            Consumer::from_hosts(hosts.to_owned())
                .with_client_id(id.to_owned())
                .with_topic("TestEvent".to_string())
                .create()?,
        ));

        let (tx, rx) = tokio::sync::mpsc::channel(MESSAGE_BUF_LEN);

        Ok(Self {
            kafka_producer: message_producer,
            kafka_consumer: message_consumer,
            event_handlers: RwLock::new(DashMap::new()),
            futures: Arc::new(Mutex::new(FuturesUnordered::new())),
            event_sender: tx,
            event_receiver: rx,
        })
    }

    /// Publish an event through Kafka
    pub async fn publish<T: Event>(&self, event: &T) -> anyhow::Result<()> {
        let ser = event.serialize()?;

        let uuid = Uuid::new_v4();
        self.event_sender
            .send(Record::from_key_value(
                type_name!(T),
                Box::leak(Box::new(uuid.as_bytes().to_owned())),
                // SAFETY: I already checked that `ser` is Some() before this call
                Box::leak(ser),
            ))
            .await
            .map_err(|_| anyhow!(EventingError::CouldNotPubilsh))?;

        // if let Err(e) = res {
        //     let record = e.0;
        //     error!(key = ?String::from_utf8_lossy(e.0.key),
        //         value = ?String::from_utf8_lossy(e.0.value),
        //         "Failed to publish record to message bus"
        //     );
        //     return Err(anyhow!(EventingError::CouldNotPubilsh {
        //         source: e
        //     }));
        // }

        Ok(())
    }

    /// Publish an event to be handled locally. Kafka will not receive this event,
    /// and the error will be sent straight back to the caller
    pub async fn publish_local<T: Event>(&self, event: &T) -> anyhow::Result<()> {
        let ser = event.serialize()?;

        let handler = self.get_handler(type_name!(T)).await?;
        handler.execute(ser).await?;

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
        let rx = self.event_receiver.recv();
        let sleep = tokio::time::sleep(Duration::from_millis(500));
        let mut futs = self.futures.lock().await;

        select! {
            Some(rec) = rx => {
                info!("Received record: {:#?}", rec);
                // TODO: add the record to a batch of records
                self.send_message(&rec).await?;
            }
            _ = sleep => {
                info!("Slept for 500 millis");
                // TODO: send all the batched messages
            }
            Some(Err(e)) = futs.next() => {
                error!(error = ?e, "Event handler returned an error");
            }
        }

        drop(futs);

        let message_sets = self.kafka_consumer.lock().await.poll()?;

        for set in message_sets.iter() {
            let topic = set.topic();
            let handler = match self.get_handler(topic).await {
                Ok(h) => h,
                Err(e) => {
                    error!(topic = ?topic, "No event handler found");
                    return Err(e);
                }
            };

            for message in set.messages() {
                let handler = handler.clone();
                let msg = message.value.to_owned();
                let fut = async move { handler.execute(msg.into_boxed_slice()).await };

                self.futures.lock().await.push(Box::pin(fut));
            }
        }

        // for res in results {
        //     if let Err(e) = res {
        //         error!(error = ?e, "Error encountered while executing event handler");
        //     }
        // }

        Ok(())
    }

    async fn send_message(&self, rec: &RecordType<'a>) -> anyhow::Result<()> {
        self.kafka_producer
            .lock()
            .await
            .send(rec)
            .map_err(|e| anyhow!(e))
    }

    async fn get_handler(
        &self,
        topic_name: &str,
    ) -> anyhow::Result<Arc<Box<dyn ErasedEventHandler + Send + Sync>>> {
        let event_handlers = self.event_handlers.read().await;
        let handler = match event_handlers.get(topic_name) {
            Some(handler) => handler.value().clone(),
            None => {
                return Err(anyhow!("No handler found for topic \"{}\"", topic_name));
            }
        };
        Ok(handler)
    }

    /// Publish all messages and advance all futures. This will also close the bus for messages,
    /// so the orchestrator may no longer publish any events
    pub async fn cleanup(&mut self) -> anyhow::Result<()> {
        self.event_receiver.close();

        let mut futs = self.futures.lock().await;
        for msg in self.event_receiver.recv().await {
            // If we do have an event handler for this, then execute it.
            if let Some(handler) = self.event_handlers.read().await.get(msg.topic) {
                let handler = handler.clone();
                let msg = msg.value.to_owned();
                let fut = async move { handler.execute(msg.into_boxed_slice()).await }.boxed();
                futs.push(fut);
            } else {
                // Otherwise, send it to Kafka
                self.send_message(&msg).await?;
            }
        }

        info!(length = ?futs.len(), "Number of futures");

        while let Some(res) = futs.next().await {
            if let Err(e) = res {
                error!(error = ?e, "Encountered error while resolving future");
            }
        }

        debug_assert!(
            futs.len() == 0,
            "Futures did not advance fully. There are still {} left",
            futs.len()
        );

        Ok(())
    }
}

// TODO: Implement Drop for the Orchestrator

/// A struct that holds data about event data
// pub struct EventMetadata<T> {
//     id: Uuid,
//     data: T,
// }

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
        cell::Cell,
        ops::AddAssign,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };

    use serde::Serialize;
    use tracing::{Instrument, Level};

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
            let mut inner = self.inner.lock().await;
            inner.add_assign(event.val);
            info!(val = ?event.val, state = ?*inner, "Adding value");
            Ok(())
        }
    }

    #[tokio::test(worker_threads = 4, flavor = "multi_thread")]
    async fn test_orchestrator_fake_event_handler() {
        tracing_subscriber::fmt()
            .with_level(true)
            .with_target(true)
            .with_max_level(tracing::Level::INFO)
            .pretty()
            .init();

        async {
            let orchestrator = Arc::new(RwLock::new(
                EventOrchestrator::new(vec!["kafka:9092".to_string()]).unwrap(),
            ));

            let handler = TestHandler::new();
            let inner = handler.inner.clone();
            let registered = handler.registered.clone();

            assert!(registered.load(Ordering::Relaxed) == false);

            orchestrator
                .write()
                .await
                .add_event_hander(Box::new(handler))
                .await
                .unwrap();

            assert!(registered.load(Ordering::Relaxed) == true);
            assert!(inner.lock().await.eq(&0));

            let (tx, rx) = tokio::sync::oneshot::channel();

            let orc = orchestrator.clone();
            tokio::spawn(
                async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    orc.read()
                        .await
                        .publish(&TestEvent { val: 1 })
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    orc.read()
                        .await
                        .publish(&TestEvent { val: 2 })
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    orc.read()
                        .await
                        .publish(&TestEvent { val: 3 })
                        .await
                        .unwrap();
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    tx.send(()).unwrap();
                }
                .instrument(debug_span!("inner")),
            );

            let mut rx = Cell::new(rx);

            // Process until finished
            loop {
                let mut orc = orchestrator.write().await;
                select! {
                    res = orc.process().instrument(info_span!("process")) => {
                        if let Err(e) = res {
                            panic!("Received error value while processing: {}", e);
                        }
                    }
                    _ = rx.get_mut() => {
                        info!("Finished processing!");
                        break;
                    }
                }
            }

            orchestrator.write().await.cleanup().await.unwrap();

            assert!(
                inner.lock().await.eq(&6),
                "Could not verify ending value: {}",
                *inner.lock().await
            );
        }
        .instrument(span!(Level::INFO, "Debug"))
        .await;
    }
}
