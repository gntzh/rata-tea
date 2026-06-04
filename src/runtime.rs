use std::any::TypeId;

use dashmap::DashMap;
use tracing::{error, trace, warn};

use crate::Sub;

/// Factory invoked at spawn-time by the runner. Consumed once.
pub mod time {
    use std::time::{Duration, Instant};

    use crate::Sub;
    pub fn every(interval: Duration) -> Sub<Instant> {
        #[derive(Hash)]
        struct Every {
            interval: Duration,
        }
        Sub::make(
            Every { interval },
            move |Every { interval }, dispatch| async move {
                let mut timer = tokio::time::interval(interval);
                loop {
                    let instant = timer.tick().await;
                    dispatch(instant.into_std()).await
                }
            },
        )
    }
}

/// Typed EventBus
#[derive(Default)]
pub struct EventBus {
    map: DashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

type BusListener<E> = (tokio::sync::mpsc::Sender<E>, Option<fn(&E) -> bool>);

#[derive(Default)]
struct Bus<E: Send + 'static> {
    listeners: Vec<BusListener<E>>,
}

impl EventBus {
    pub fn new() -> Self {
        Default::default()
    }

    /// Interior mutability
    pub fn subscribe<E: Send + 'static>(
        &self,
        filter: impl Into<Option<fn(&E) -> bool>>,
    ) -> tokio::sync::mpsc::Receiver<E> {
        let (tx, rx) = tokio::sync::mpsc::channel(128);
        let mut entry = self
            .map
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(Bus::<E> { listeners: vec![] }));
        let bus = entry.downcast_mut::<Bus<E>>().expect("got by TypeId");
        bus.listeners.push((tx, filter.into()));
        trace!(
            "Adding a subscriber of Event<{:?}>, the total number will be {}.",
            TypeId::of::<E>(),
            bus.listeners.len()
        );
        rx
    }

    pub async fn publish<E: Send + 'static + Clone>(&self, event: E) {
        if let Some(bus) = self
            .map
            .get_mut(&TypeId::of::<E>())
            .as_mut()
            .and_then(|entry| entry.downcast_mut::<Bus<E>>())
        {
            // the channel is closed when `UnboundedReceiver` is dropped,
            // that is, when the stream created by the sub that holds the `UnboundedReceiver` is dropped.
            bus.listeners.retain(|listener| !listener.0.is_closed());
            if let Some((last, rest)) = bus
                .listeners
                .iter()
                .filter(|(_, filter)| filter.map(|f| f(&event)).unwrap_or(true))
                .collect::<Vec<_>>()
                .split_last()
            {
                fn logging_send_event_result<E>(
                    rst: Result<(), tokio::sync::mpsc::error::SendError<E>>,
                ) {
                    if let Err(err) = rst {
                        error!(?err, "event channel was closed unexpectedly.")
                    };
                }
                for tx in rest {
                    trace!("cloning event of {:?}", TypeId::of::<E>());
                    logging_send_event_result(tx.0.send(event.clone()).await)
                }
                logging_send_event_result(last.0.send(event.clone()).await);
            }
        }
    }
}

impl EventBus {
    #[inline]
    pub fn sub<E: Send + 'static>(&self, filter: impl Into<Option<fn(&E) -> bool>>) -> Sub<E> {
        let mut rx = self.subscribe(filter);
        Sub::make((), move |(), dispatch| async move {
            loop {
                match rx.recv().await {
                    Some(event) => {
                        dispatch(event).await;
                    }
                    None => warn!("Event channel is closed by Sender"),
                }
            }
        })
    }
}
