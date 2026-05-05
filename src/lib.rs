use std::{any::TypeId, fmt::Debug, hash::Hash};

pub use crossterm::event::Event as TerminalEvent;
use dashmap::DashMap;
use tracing::{error, trace};

pub use core::*;
pub use runner::{Action, Runner};
pub use tea::*;

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
        Sub::make(Every { interval }, move |Every { interval }, dispatch| {
            let (fut, handle) = futures::future::abortable(async move {
                let mut timer = tokio::time::interval(interval);
                loop {
                    let instant = timer.tick().await;
                    dispatch(instant.into_std())
                }
            });
            tokio::spawn(fut);
            handle
        })
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
                let rx = GLOBAL_EVENT_BUS.subscribe::<TerminalEvent>(None);
                let (fut, handle) = futures::future::abortable(async move {
                    loop {
                        match rx.recv().await {
                            Ok(event) => {
                                dispatch(event);
                            }
                            Err(err) => warn!(?err, "Event is closed by Sender"),
                        }
                    }
                });
                tokio::spawn(fut);
                handle
            },
        )
    }

    pub fn on_key_event() -> Sub<KeyEvent> {
        Sub::make((), |(), dispatch| {
            let rx = GLOBAL_EVENT_BUS.subscribe::<TerminalEvent>(None);
            let (fut, handle) = futures::future::abortable(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if let TerminalEvent::Key(event @ KeyEvent { .. }) = event {
                                dispatch(event);
                            }
                        }
                        Err(err) => warn!(?err, "Event is closed by Sender"),
                    }
                }
            });
            tokio::spawn(fut);
            handle
        })
    }

    pub fn on_key_press() -> Sub<KeyEvent> {
        Sub::make((), |(), dispatch| {
            let rx = GLOBAL_EVENT_BUS.subscribe::<TerminalEvent>(None);
            let (fut, handle) = futures::future::abortable(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if let TerminalEvent::Key(
                                event @ KeyEvent {
                                    kind: KeyEventKind::Press,
                                    ..
                                },
                            ) = event
                            {
                                dispatch(event);
                            }
                        }
                        Err(err) => warn!(?err, "Event is closed by Sender"),
                    }
                }
            });
            tokio::spawn(fut);
            handle
        })
    }
}

/// Typed EventBus
#[derive(Default)]
pub struct EventBus {
    map: DashMap<TypeId, Box<dyn std::any::Any + Send + Sync>>,
}

type BusListener<E> = (async_channel::Sender<E>, Option<fn(&E) -> bool>);

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
    ) -> async_channel::Receiver<E> {
        let (tx, rx) = async_channel::bounded(100);
        let mut entry = self
            .map
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(Bus::<E> { listeners: vec![] }));
        let bus = entry.downcast_mut::<Bus<E>>().expect("got by TypeId");
        bus.listeners.push((tx, filter.into()));
        trace!(
            "Adding a subscriber of {:?}, the total number will be {}.",
            TypeId::of::<E>(),
            bus.listeners.len()
        );
        rx
    }

    pub fn publish<E: OwnedSend + Clone>(&self, event: E) {
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
                    rst: Result<Option<E>, async_channel::SendError<E>>,
                ) {
                    match rst {
                        Ok(Some(_)) => error!(
                            "too many event in channel, drop an oldest msg. This should not happen here."
                        ),
                        Err(_) => error!("event channel was closed unexpectedly."),
                        Ok(None) => (),
                    };
                }
                for tx in rest {
                    trace!("cloning event of {:?}", TypeId::of::<E>());
                    logging_send_event_result(tx.0.force_send(event.clone()))
                }
                logging_send_event_result(last.0.force_send(event.clone()));
            }
        }
    }
}

// pub struct Task<T, E>(Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'static>>);

// impl<T: 'static, E: 'static> Task<T, E> {
//     pub fn future(fut: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
//         Self(Box::pin(fut))
//     }

//     /// run a
//     pub fn attempt<Msg, F>(self, mapper: F) -> Cmd<Msg>
//     where
//         F: FnOnce(Result<T, E>) -> Msg + Send + 'static,
//         Msg: OwnedSend,
//     {
//         Cmd::future(async move { mapper(self.0.await) })
//     }
// }

// impl<T: 'static> Task<T, Infallible> {
//     pub fn perform<Msg, F>(self, mapper: F) -> Cmd<Msg>
//     where
//         F: FnOnce(T) -> Msg + Send + 'static,
//         Msg: OwnedSend,
//     {
//         Cmd::future(async move { mapper(self.0.await.expect("unwrap from an infallible result")) })
//     }
// }
