pub use crossterm::event::Event as TerminalEvent;

pub use core::*;
pub use runner::{Action, Runner};
pub use runtime::time;
pub use tea::*;

pub mod runtime;

mod core;
mod tea;

mod runner;

pub mod terminal {
    use std::sync::LazyLock;

    use crossterm::event::{KeyEvent, KeyEventKind};
    use tracing::warn;

    use crate::{Dispatch, Sub, TerminalEvent, runtime::EventBus};

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
