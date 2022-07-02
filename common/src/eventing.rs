use std::{
    any::{Any, TypeId},
    collections::LinkedList,
    pin::Pin,
    process::Output,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use dashmap::DashMap;
use futures::{Future, FutureExt, Stream};
use sea_orm::prelude::Uuid;
use tokio::task::JoinHandle;

/// TODO
///
/// Event types:
///  - Oneshot (executes once and then is removed from the event handlers)
///  - Single (executes once per event)
///  - Batch (executes once with many Events)

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
        }

        Ok(())
    }

    pub async fn publish<T>(&self, event: T) -> anyhow::Result<()>
    where
        T: Event + Any + 'static,
    {
        if let Some(handlers) = self.handlers.get(&TypeId::of::<T>()) {
            let data = event.get_data();
            for handler in handlers.iter() {
                // handler.execute(data).await?;
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

impl Drop for Orchestrator {
    fn drop(&mut self) {
        for mut handlers in self.handlers.iter_mut() {
            for handler in handlers.iter_mut() {
                handler.unregister().unwrap();
            }
        }
    }
}

// Contains the information and execution context for events of a specific type.
// This will also downcast the inner `ErasedEventHandler` into
// the proper EventHandler type and handle it accordingly.
struct EventExecutor {
    handlers: Vec<HandlerMetadata>,

    /// A stream of events that are ready to be processed
    /// Whenever events are being processed, the `events` Vec will be swapped with
    /// an empty one, and the events will be processed by all the available handlers
    events: Vec<Box<dyn Any + Send + Sync>>,
    recycled_vec: Vec<Box<dyn Any + Send + Sync>>,

    is_modifying_events: AtomicBool,
}

impl EventExecutor {
    // Let all the event handlers run, and then execute the new events
    pub async fn execute_events(&mut self) -> anyhow::Result<()> {
        let results = futures::future::join_all(
            (&mut self.handlers)
                .into_iter()
                .filter_map(|h| h.future.as_mut().take()),
        )
        .await;

        let errs = results
            .into_iter()
            .filter_map(|res| match res {
                Ok(output) => {
                    if let Err(e) = output {
                        return Some(e.to_string());
                    }

                    return None;
                }
                Err(e) => Some(e.to_string()),
            })
            .collect::<Vec<String>>();

        if errs.len() > 0 {
            bail!(
                "Encountered errors while processing events: {}",
                errs.into_iter().fold(String::new(), |mut buf, err| {
                    buf.push_str(err.as_str());
                    buf
                })
            );
        }

        // Swap the old and new vecs

        self.recycled_vec.clear();

        let mut res = Err(false);
        while res.is_err() {
            res = self.is_modifying_events.compare_exchange(
                false,
                true,
                Ordering::Acquire,
                Ordering::Acquire,
            );
        }

        let old_vec = self.events.as_mut_ptr();
        let old_vec_size = self.events.len();
        let old_vec_cap = self.events.capacity();

        self.events = unsafe {
            Vec::from_raw_parts(
                self.recycled_vec.as_mut_ptr(),
                self.recycled_vec.len(),
                self.recycled_vec.capacity(),
            )
        };
        self.recycled_vec = unsafe { Vec::from_raw_parts(old_vec, old_vec_size, old_vec_cap) };

        self.is_modifying_events.store(false, Ordering::Release);

        for handler in &self.handlers {
            handler.handler.execute(
                Box::new(self.events.as_slice().into_iter()) as Box<dyn Iterator<Item = Box<dyn Any + Send + Sync>> + Send + Sync>
            );
        }

        Ok(())
    }

    // fn advance_futures(&mut self) -> anyhow::Result<()> {
    //     let handlers = &mut self.executing_handlers;
    //     let mut handlers_to_remove = Vec::new();
    //     for (i, fut) in handlers.into_iter().enumerate() {
    //         if fut.is_finished() {
    //             handlers_to_remove.push(i);
    //         }
    //     }

    //     // Reverse sort these keys
    //     handlers_to_remove.sort_by(|a, b| b.cmp(a));

    //     let rt = tokio::runtime::Handle::current();

    //     for index in handlers_to_remove {
    //         let fut = handlers.swap_remove(index);

    //         // This could _possibly_ be a race condition, and there's a reason that it's a debug check
    //         debug_assert!(fut.is_finished());

    //         // TODO: Handle errors instead of ending execution of this function
    //         rt.block_on(fut)??;
    //     }

    //     Ok(())
    // }
}

struct HandlerMetadata {
    // TODO: Find a faster implementation than using UUIDs
    id: Uuid,
    handler: Box<dyn ErasedEventHandler + Send + Sync + 'static>,
    future: Option<Box<JoinHandle<anyhow::Result<()>>>>,
}

pub trait Event {
    /// Can be set to () if there's no data
    type EventData: Sized;

    fn get_data(&self) -> Self::EventData;
}

#[async_trait]
pub trait EventHandler {
    type Event: Event;

    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    /// Batch process any incoming events
    async fn execute(
        &self,
        data: Box<dyn Iterator<Item = <Self::Event as Event>::EventData> + Send + Sync>,
    ) -> anyhow::Result<()>;
}

#[async_trait]
trait ErasedEventHandler: Any {
    /// If the EventHandler fails to register, the event handler will not be added.
    fn register(&mut self) -> anyhow::Result<()>;

    fn unregister(&mut self) -> anyhow::Result<()>;

    async fn execute(
        &self,
        data: Box<dyn Iterator<Item = Box<dyn Any + Send + Sync>> + Send + Sync>,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl<H> ErasedEventHandler for H
where
    H: EventHandler + Send + Sync + 'static,
{
    fn register(&mut self) -> anyhow::Result<()> {
        println!("Registering...");
        self.register()
    }

    fn unregister(&mut self) -> anyhow::Result<()> {
        println!("Unregistering...");
        self.unregister()
    }

    async fn execute(
        &self,
        _data: Box<dyn Iterator<Item = Box<dyn Any>> + Send + Sync>,
    ) -> anyhow::Result<()> {
        println!("Executing...");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_fake_event_handler() {
        struct TestEvent;

        impl Event for TestEvent {
            type EventData = ();

            fn get_data(&self) -> Self::EventData {
                ()
            }
        }

        struct TestHandler {
            pub registered: &'static mut bool,
        }

        #[async_trait]
        impl EventHandler for TestHandler {
            type Event = TestEvent;

            async fn execute(
                &self,
                data: Box<dyn Iterator<Item = <Self::Event as Event>::EventData> + Send + Sync>,
            ) -> anyhow::Result<()> {
                for ev in data {
                    unsafe {
                        COUNTER += 1;
                    }
                }
                Ok(())
            }

            fn register(&mut self) -> anyhow::Result<()> {
                *self.registered = true;
                Ok(())
            }

            fn unregister(&mut self) -> anyhow::Result<()> {
                *self.registered = false;
                Ok(())
            }
        }

        static mut COUNTER: u32 = 0;

        let mut orchestrator = Orchestrator {
            handlers: DashMap::new(),
        };

        static mut REGISTERED: bool = false;
        let handler = unsafe {
            TestHandler {
                registered: &mut REGISTERED,
            }
        };

        orchestrator.insert(handler).unwrap();

        unsafe {
            assert!(REGISTERED == true);
        }

        orchestrator.publish(TestEvent {}).await.unwrap();
        orchestrator.publish(TestEvent {}).await.unwrap();
        orchestrator.publish(TestEvent {}).await.unwrap();
        orchestrator.publish(TestEvent {}).await.unwrap();

        unsafe {
            assert!(COUNTER == 4);
        }

        drop(orchestrator);

        unsafe {
            assert!(REGISTERED == false);
        }
    }
}
