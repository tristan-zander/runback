use cqrs_es::{Aggregate, EventEnvelope, Query};
use serde::Serialize;

pub struct DiscordEventQuery {}

#[async_trait]
impl<T: Aggregate + Serialize> Query<T> for DiscordEventQuery {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<T>]) {
        for event in events {
            debug!(
                "Received event: {} {}",
                aggregate_id,
                serde_json::to_string(&event.payload).unwrap()
            );
        }
    }
}
