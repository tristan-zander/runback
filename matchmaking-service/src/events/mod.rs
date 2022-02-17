use std::{error::Error, time::Duration};

use rdkafka::{
    message::OwnedHeaders,
    producer::{FutureProducer, FutureRecord},
    ClientConfig,
};
use rocket::futures::future::join_all;

pub struct EventLoop {
    producer: FutureProducer,
}

impl EventLoop {
    pub fn new(event_settings: crate::config::Events) -> Result<EventLoop, Box<dyn Error>> {
        let mut kafka_config = ClientConfig::new();

        for (key, val) in event_settings.kafka_settings {
            debug!(key = %key, val = %val, message = "Adding configuration key-value pair to Kafka producer");
            kafka_config.set(key, val);
        }

        let producer: FutureProducer = kafka_config.create()?;

        Ok(EventLoop { producer })
    }

    #[allow(dead_code)]
    pub async fn run_event_loop(&mut self) {}

    pub async fn fake_event_loop(&mut self) {
        // This loop is non blocking: all messages will be sent one after the other, without waiting
        // for the results.
        let producer = &self.producer;
        let futures = (0..5)
            .map(|i| async move {
                // The send operation on the topic returns a future, which will be
                // completed once the result or failure from Kafka is received.
                let delivery_status = producer
                    .send(
                        FutureRecord::to("example_topic")
                            .payload(&format!("Message {}", i))
                            .key(&format!("Key {}", i))
                            .headers(OwnedHeaders::new().add("header_key", "header_value")),
                        Duration::from_secs(0),
                    )
                    .await;

                // This will be executed when the result is received.
                info!("Delivery status for message {} received", i);
                delivery_status
            })
            .collect::<Vec<_>>();

        let f = join_all(futures).await;
        for val in f.iter() {
            info!("Future completed. Result: {:?}", val);
        }
    }
}
