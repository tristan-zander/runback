use std::{
    any::{Any},
    time::Duration,
};

use dashmap::DashMap;
use kafka::{
    consumer::Consumer,
    producer::{Producer, Record},
};
use sea_orm::prelude::Uuid;
use serde::{Deserialize};
use tokio::sync::{Mutex, RwLock};


pub struct Orchestrator<'a> {
    message_producer: Producer,
    message_consumer: Consumer,
    outbound_messages: Mutex<Vec<Record<'a, &'a [u8], &'a [u8]>>>,
    // Only one event handler per event type
    event_handlers: RwLock<DashMap<&'static str, Box<dyn ErasedEventHandler>>>,
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
        })
    }

    /// Publish an event through Kafka
    pub async fn publish<T: Event>(&self, event: &T) {
        let mut messages = self.outbound_messages.lock().await;

        messages.push(Record::<'a>::from_key_value(
            std::any::type_name::<T>().split("::").last().unwrap(),
            Box::leak(Box::new(Uuid::new_v4().as_bytes().to_owned())),
            Box::leak(event.get_data()),
        ));
    }

    /// Add an event handler to the orchestrator
    pub async fn add_event_hander<T>(&self, mut handler: Box<T>) -> anyhow::Result<()>
    where
        T: EventHandler + Send + Sync + 'static,
    {
        handler.register()?;

        let _old = self.event_handlers.write().await.insert(
            std::any::type_name::<T::Event>()
                .split("::")
                .last()
                .unwrap(),
            handler,
        );

        Ok(())
    }

    /// Waits for an event given a certain criteria
    pub async fn wait_for_event<T: Event, F: Fn() -> ()>(&self, _f: F) -> () {}

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
                handler.execute(&message.value)?;
            }
        }

        Ok(())
    }
}

impl<'a> Drop for Orchestrator<'a> {
    fn drop(&mut self) {
        if let Ok(handlers) = self.event_handlers.try_write() {
            for mut handler in handlers.iter_mut() {
                let handler = handler.value_mut();
                match handler.unregister() {
                    Ok(_) => {}
                    Err(e) => error!("Failure to unregister handler: {}", e),
                }
            }
        } else {
            panic!("Could not de-register event handlers.");
        }
    }
}

pub trait Event: Deserialize<'static> {
    fn get_data(&self) -> Box<[u8]>;
}

pub trait EventHandler {
    type Event: Event;

    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    fn execute(&self, event: &Self::Event) -> anyhow::Result<()>;
}

pub(crate) trait ErasedEventHandler: Any {
    /// If the EventHandler fails to register, the event handler will not be added.
    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    fn execute(&self, data: &[u8]) -> anyhow::Result<()>;
}

impl<H> ErasedEventHandler for H
where
    H: EventHandler + 'static,
{
    fn register(&mut self) -> anyhow::Result<()> {
        trace!(action = ?"register", handler = ?std::any::type_name::<H>());
        self.register()
    }

    fn unregister(&mut self) -> anyhow::Result<()> {
        trace!(action = ?"unregister", handler = ?std::any::type_name::<H>());
        self.unregister()
    }

    fn execute(&self, data: &[u8]) -> anyhow::Result<()> {
        trace!(
            handler = ?std::any::type_name::<H>(),
            data = ?String::from_utf8_lossy(data)
        );

        // TODO: Figure out how to do this without copying.
        let boxed = data.to_owned().into_boxed_slice();
        let data = serde_json::de::from_slice(Box::leak(boxed))?;

        self.execute(&data)?;

        Ok(())

        // if let Some(data) = data.downcast_ref() {
        //     return self.execute(data);
        // }

        // Err(anyhow!(
        //     "Could not downcast the event data to the proper type"
        // ))
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

    impl Event for TestEvent {
        fn get_data(&self) -> Box<[u8]> {
            let ret = match serde_json::to_vec(self) {
                Ok(v) => kafka::producer::AsBytes::as_bytes(&v).to_owned().into_boxed_slice(),
                Err(_) => Box::new([]),
            };
            ret
        }
    }

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

        fn execute(&self, event: &Self::Event) -> anyhow::Result<()> {
            loop {
                let mut lock = match self.inner.try_lock() {
                    Ok(lock) => lock,
                    Err(_) => {
                        continue;
                    }
                };
                lock.add_assign(event.val);
                break;
            }

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

        orchestrator.publish(&TestEvent { val: 1 }).await;
        orchestrator.publish(&TestEvent { val: 2 }).await;
        orchestrator.publish(&TestEvent { val: 3 }).await;

        assert!(registered.load(Ordering::Relaxed) == true);
        assert!(inner.lock().await.eq(&0));

        orchestrator.process().await.unwrap();

        assert!(inner.lock().await.eq(&6));
    }
}
