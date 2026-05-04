use std::hash::Hasher as _;

use futures::{StreamExt, stream};
use tokio::{sync::mpsc, task::JoinHandle};
use tokio_stream::StreamMap;
use tracing::{debug, trace, warn};

use crate::{Action, Cmd, Event, EventBus, Hasher, OwnedSend, MsgStream, Sub, Tea, Tick};

pub struct Runner {
    frame_rate: f64,
}

impl Runner {
    pub fn frame_rate(self, frame_rate: f64) -> Self {
        Self { frame_rate }
    }

    #[tokio::main]
    pub async fn run<T: Tea>(self, tea: T) -> color_eyre::Result<()> {
        color_eyre::install()?;
        let mut tui = ratatui::try_init()?;

        let mut event_bus = EventBus::new();
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<Action<T::Msg>>();

        let tick_stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
            std::time::Duration::from_secs_f64(1.0 / self.frame_rate),
        ))
        .map(|_| Ok(Event::Tick(Tick)))
        .boxed();
        let terminal_stream = crossterm::event::EventStream::new()
            .map(|event| event.map(Event::Terminal))
            .boxed();
        let mut event_stream = stream::select_all([tick_stream, terminal_stream]);

        let (mut model, cmd) = tea.init();
        let mut active_sub = StreamMap::new();

        Self::rebuild_sub(tea.subscriptions(&model), &mut event_bus, &mut active_sub);
        Self::spawn_cmd(cmd, action_tx.clone());
        let mut dirty = true;

        loop {
            tokio::select! {
                Some((_, msg)) = active_sub.next() => {
                    action_tx
                        .send(Action::Msg(msg))
                        .expect("action/msg channel was closed unexpectedly");
                }
                Some(action) = action_rx.recv() => {
                    match action {
                        Action::Quit => break,
                        Action::SendEvent(sender) => sender(&event_bus),
                        Action::Msg(msg) => {
                            let cmd = tea.update(&mut model, msg);
                            dirty = true;
                            Self::rebuild_sub(tea.subscriptions(&model), &mut event_bus, &mut active_sub);
                            Self::spawn_cmd(cmd, action_tx.clone());
                        },
                    }
                }
                Some(event) = event_stream.next() => {
                    match event {
                        Ok(event) => {
                            if dirty && let Event::Tick(_) = event  {
                                trace!("drawing frame");
                                tui.draw(|frame| tea.view(&mut model, frame)).expect("terminal draw failed");
                                dirty = false;
                            }
                            match event {
                                Event::Tick(tick) => event_bus.publish(tick),
                                Event::Terminal(term_event) => event_bus.publish(term_event),
                            }
                        },
                        Err(err) => {
                            warn!(?err, "error reading terminal event");
                        }
                    }
                }
            };
        }

        ratatui::try_restore()?;
        Ok(())
    }

    fn spawn_cmd<M: OwnedSend>(
        cmd: Cmd<M>,
        action_tx: mpsc::UnboundedSender<Action<M>>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            for cmd in cmd.0 {
                tokio::spawn({
                    let action_tx = action_tx.clone();
                    async move {
                        let action = cmd.await;
                        action_tx
                            .send(action)
                            .expect("action/msg channel was closed unexpectedly");
                    }
                });
            }
        })
    }

    fn rebuild_sub<M: OwnedSend>(
        sub: Sub<M>,
        event_bus: &mut EventBus,
        active_sub: &mut StreamMap<u64, MsgStream<M>>,
    ) {
        let mut new = StreamMap::new();
        for factory in sub.0 {
            let mut hasher = Hasher::new();
            factory.hash(&mut hasher);
            let hash = hasher.finish();
            if let Some(stream) = active_sub.remove(&hash) {
                new.insert(hash, stream);
            } else {
                trace!(?hash, "creating new subscription stream");
                new.insert(hash, factory.stream(event_bus.sub_cap()));
            }
        }
        *active_sub = new;
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self { frame_rate: 30.0 }
    }
}
