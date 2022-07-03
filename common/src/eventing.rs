use std::time::Duration;

use kafka::{
    client::KafkaClient,
    consumer::Consumer,
    producer::{AsBytes, Producer, Record},
};
use sea_orm::prelude::Uuid;
use tokio::sync::Mutex;

pub struct Orchestrator<'a> {
    message_producer: Producer,
    message_consumer: Consumer,
    message_queue: Mutex<Vec<Record<'a, &'a [u8], &'a [u8]>>>,
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
            .with_topic("test-topic".to_string())
            .create()?;

        Ok(Self {
            message_producer,
            message_queue: Mutex::new(Vec::with_capacity(128)),
            message_consumer,
        })
    }

    /// Publish an event through Kafka
    pub async fn publish<T: Event>(&self, event: &T) {
        let mut messages = self.message_queue.lock().await;

        messages.push(Record::<'a>::from_key_value(
            event.topic_name(),
            Box::leak(Box::new(Uuid::new_v4().as_bytes().to_owned())),
            Box::leak(event.get_data()),
        ));
    }

    /// Add an event handler to the orchestrator
    pub fn add_event_hander<T>(&self) {}

    /// Waits for an event given a certain criteria
    pub async fn wait_for_event<T: Event, F: Fn() -> ()>(&self, _f: F) -> () {}

    /// Fetch any incoming events and dispatch any event handlers.
    pub async fn process(&mut self) -> anyhow::Result<()> {
        let mut messages = self.message_queue.lock().await;

        let res = self.message_producer.send_all(messages.as_slice())?;

        println!("{:#?}", res);

        messages.clear();

        Ok(())
    }
}

pub trait Event {
    // TODO: Make this a compile-time computed value based on the name of the Event
    fn topic_name(&self) -> &'static str;

    fn get_data(&self) -> Box<[u8]>;
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct TestEvent {
        pub val: String,
    }

    impl Event for TestEvent {
        fn topic_name(&self) -> &'static str {
            "test-topic"
        }

        fn get_data(&self) -> Box<[u8]> {
            let ret = match serde_json::to_vec(self) {
                Ok(v) => v.as_bytes().to_owned().into_boxed_slice(),
                Err(_) => Box::new([]),
            };
            ret
        }
    }

    #[tokio::test(worker_threads = 4, flavor = "multi_thread")]
    async fn test_orchestrator_fake_event_handler() {
        let mut orchestrator = Orchestrator::new(vec!["kafka:9092".to_string()]).unwrap();

        orchestrator
            .publish(&TestEvent {
                val: "Test 1".to_string(),
            })
            .await;
        orchestrator
            .publish(&TestEvent {
                val: "Test 2".to_string(),
            })
            .await;
        orchestrator
            .publish(&TestEvent {
                val: "Test 3".to_string(),
            })
            .await;

        orchestrator.process().await.unwrap();
    }
}
