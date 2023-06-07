use cqrs_es::{Query, EventEnvelope, Aggregate};
use serde::Serialize;

use crate::events::Lobby;


pub struct DiscordEventQuery {

}

#[async_trait]
impl<T: Aggregate + Serialize> Query<T> for DiscordEventQuery {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<T>]) {
        for event in events {
            debug!("Received event: {} {}",  aggregate_id, serde_json::to_string(&event.payload).unwrap());
        }
    }
}