use std::{any::TypeId, fmt::Debug, hash::Hash};

pub use crossterm::event::Event as TerminalEvent;
use dashmap::DashMap;
use tracing::{error, trace};

pub use core::*;
pub use tea::*;
pub use runner::{Action, Runner};

mod core;
mod runner;
mod tea;

#[derive(Debug, Clone, strum::EnumDiscriminants, strum::EnumIs)]
#[strum_discriminants(derive(Hash))]
#[strum_discriminants(name(EventType))]
pub enum Event {
    Tick(Tick),
    Terminal(TerminalEvent),
}

#[derive(Debug, Clone)]
pub struct Tick;

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

pub mod terminal {
    use std::sync::LazyLock;

    use crossterm::event::{KeyEvent, KeyEventKind};
    use tracing::warn;

    use crate::{Dispatch, EventBus, Sub, TerminalEvent};

    pub static GLOBAL_EVENT_BUS: LazyLock<EventBus> = LazyLock::new(EventBus::default);

    pub fn on_term_event() -> Sub<TerminalEvent> {
        Sub::make(
            (),
            |(), dispatch: Box<dyn Dispatch<TerminalEvent> + 'static>| {
                let mut rx = GLOBAL_EVENT_BUS.subscribe::<TerminalEvent>(None);
                async move {
                    loop {
                        match rx.recv().await {
                            Some(event) => {
                                dispatch(event).await;
                            }
                            None => warn!("Event channel is closed by Sender"),
                        }
                    }
                }
            },
        )
    }

    pub fn on_key_event() -> Sub<KeyEvent> {
        Sub::make((), |(), dispatch| {
            let mut rx = GLOBAL_EVENT_BUS.subscribe::<TerminalEvent>(None);
            async move {
                loop {
                    match rx.recv().await {
                        Some(event) => {
                            if let TerminalEvent::Key(event @ KeyEvent { .. }) = event {
                                dispatch(event).await;
                            }
                        }
                        None => warn!("Event channel is closed by Sender"),
                    }
                }
            }
        })
    }

    pub fn on_key_press() -> Sub<KeyEvent> {
        Sub::make((), |(), dispatch| {
            let mut rx = GLOBAL_EVENT_BUS.subscribe::<TerminalEvent>(None);
            async move {
                loop {
                    match rx.recv().await {
                        Some(event) => {
                            if let TerminalEvent::Key(
                                event @ KeyEvent {
                                    kind: KeyEventKind::Press,
                                    ..
                                },
                            ) = event
                            {
                                dispatch(event).await;
                            }
                        }
                        None => warn!("Event channel is closed by Sender"),
                    }
                }
            }
        })
    }
}

/// Typed EventBus
#[derive(Default)]
pub struct EventBus {
    map: DashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

type BusListener<E> = (tokio::sync::mpsc::Sender<E>, Option<fn(&E) -> bool>);

#[derive(Default)]
struct Bus<E: OwnedSend> {
    listeners: Vec<BusListener<E>>,
}

impl EventBus {
    pub fn new() -> Self {
        Default::default()
    }

    /// Interior mutability
    pub fn subscribe<E: OwnedSend>(
        &self,
        filter: impl Into<Option<fn(&E) -> bool>>,
    ) -> tokio::sync::mpsc::Receiver<E> {
        let (tx, rx) = tokio::sync::mpsc::channel(100);
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

    pub async fn publish<E: OwnedSend + Clone>(&self, event: E) {
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
