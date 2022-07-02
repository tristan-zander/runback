use kafka::client::KafkaClient;

pub struct Orchestrator {
    kafka_client: KafkaClient,
}

impl Orchestrator {
    /// Publish an event through Kafka
    pub fn publish<T>(&self) {}

    /// Add an event handler to the orchestrator
    pub fn add_event_hander<T>(&self) {}

    /// Waits for an event given a certain criteria
    pub async fn wait_for_event<T, F: Fn() -> ()>(&self, _f: F) -> () {}

    /// Fetch any incoming events and dispatch any event handlers.
    pub async fn process(&mut self) {
        // self.kafka_client
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(worker_threads = 4)]
    async fn test_orchestrator_fake_event_handler() {}
}
