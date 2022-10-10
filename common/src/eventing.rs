use std::any::{Any, TypeId};

use dashmap::DashMap;

pub struct Orchestrator {
    handlers: DashMap<
        // Where "TypeId" is the type of the event
        TypeId,
        Vec<Box<dyn ErasedEventHandler + Send + Sync + 'static>>,
    >,
}

impl Orchestrator {
    pub fn insert<H>(&mut self, mut handler: H) -> anyhow::Result<()>
    where
        H: EventHandler + Send + Sync + 'static,
    {
        handler.register()?;

        let boxed = Box::new(handler);
        let type_id = TypeId::of::<H::Event>();
        if let Some(mut handlers) = self.handlers.get_mut(&type_id) {
            handlers.push(boxed as Box<dyn ErasedEventHandler + Send + Sync>);
        } else {
            self.handlers.insert(type_id, vec![boxed]);
            // .ok_or(anyhow!("Could not insert handler into map"))?;
        }

        Ok(())
    }

    pub fn publish<T>(&self, event: T) -> anyhow::Result<()>
    where
        T: Event + Any + 'static,
    {
        if let Some(handlers) = self.handlers.get(&TypeId::of::<T>()) {
            let data = event.get_data();
            for handler in handlers.iter() {
                handler.execute(data)?;
            }
        } else {
            return Err(anyhow!(
                "Could not get the handler with TypeId of {:#?}",
                TypeId::of::<T>()
            ));
        }

        // Otherwise, no handlers have been registered.

        Ok(())
    }
}

pub trait Event {
    // Can be set to () if there's no data
    type EventData: Sized;

    fn get_data(&self) -> &Self::EventData;
}

pub trait EventHandler {
    type Event: Event;
    /// If the EventHandler fails to register, the event handler will not be added.
    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    fn execute(&self, data: &<Self::Event as Event>::EventData) -> anyhow::Result<()>;
}

trait ErasedEventHandler: Any {
    /// If the EventHandler fails to register, the event handler will not be added.
    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    fn execute(&self, data: &dyn Any) -> anyhow::Result<()>;
}

impl<H> ErasedEventHandler for H
where
    H: EventHandler + 'static,
{
    fn register(&mut self) -> anyhow::Result<()> {
        println!("Registering...");
        self.register()
    }

    fn unregister(&mut self) -> anyhow::Result<()> {
        println!("Unregistering...");
        self.unregister()
    }

    fn execute(&self, data: &dyn Any) -> anyhow::Result<()> {
        println!("Executing...");
        if let Some(data) = data.downcast_ref() {
            return self.execute(data);
        }

        Err(anyhow!(
            "Could not downcast the event data to the proper type"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_fake_event_handler() {
        struct TestEvent;

        impl Event for TestEvent {
            type EventData = ();

            fn get_data(&self) -> &Self::EventData {
                &()
            }
        }

        struct TestHandler {
            pub registered: bool,
        }

        impl EventHandler for TestHandler {
            type Event = TestEvent;

            fn register(&mut self) -> anyhow::Result<()> {
                self.registered = true;
                Ok(())
            }

            fn unregister(&mut self) -> anyhow::Result<()> {
                self.registered = false;
                Ok(())
            }

            fn execute(&self, _data: &<Self::Event as Event>::EventData) -> anyhow::Result<()> {
                unsafe {
                    COUNTER += 1;
                }
                Ok(())
            }
        }

        static mut COUNTER: u32 = 0;

        let mut orchestrator = Orchestrator {
            handlers: DashMap::new(),
        };

        let handler = TestHandler { registered: false };

        orchestrator.insert(handler).unwrap();

        orchestrator.publish(TestEvent {}).unwrap();
        orchestrator.publish(TestEvent {}).unwrap();
        orchestrator.publish(TestEvent {}).unwrap();
        orchestrator.publish(TestEvent {}).unwrap();

        unsafe {
            assert!(COUNTER == 4);
        }
    }
}
