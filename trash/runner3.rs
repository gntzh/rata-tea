use std::hash::Hasher as _;

use futures::{StreamExt, stream};
use tokio::{
    sync::{broadcast::Receiver, mpsc::UnboundedSender},
    task::JoinHandle,
};
use tokio_stream::StreamMap;
use tracing::{debug, warn};

use crate::{Action, Cmd, Event, Hasher, Msg, MsgStream, Sub, Tea};

pub struct Runner {
    frame_rate: f64,
}

impl Runner {
    pub fn frame_rate(self, frame_rate: f64) -> Self {
        Self { frame_rate, ..self }
    }

    #[tokio::main]
    pub async fn run<T: Tea>(self, tea: T) -> color_eyre::Result<()> {
        color_eyre::install()?;
        let mut tui = ratatui::try_init()?;

        let (event_tx, event_rx) = tokio::sync::broadcast::channel(100);
        let (action_tx, mut action_rx) = tokio::sync::mpsc::unbounded_channel::<Action<T::Msg>>();

        let frame_stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
            std::time::Duration::from_secs_f64(1.0 / self.frame_rate),
        ))
        .map(|_| Ok(Event::Frame))
        .boxed();
        let terminal_stream = crossterm::event::EventStream::new()
            .map(|event| event.map(Event::Terminal))
            .boxed();
        let mut event_stream = stream::select_all([frame_stream, terminal_stream]);

        let (mut model, cmd) = tea.init();
        let mut sub = StreamMap::new();

        Self::rebuild_sub(tea.subscriptions(&model), &event_rx, &mut sub);
        Self::spawn_cmd(cmd, action_tx.clone());
        let mut dirty = true;

        loop {
            tokio::select! {
                Some((_, msg)) = sub.next() => {
                    action_tx
                        .send(Action::Msg(msg))
                        .expect("action/msg channel was closed unexpectedly");
                }
                Some(action) = action_rx.recv() => {
                    match action {
                        Action::Quit => break,
                        Action::Msg(msg) => {
                            let cmd = tea.update(&mut model, msg);
                            dirty = true;
                            Self::rebuild_sub(tea.subscriptions(&model), &event_rx, &mut sub);
                            Self::spawn_cmd(cmd, action_tx.clone());
                        },
                    }
                }
                Some(event) = event_stream.next() => {
                    match event {
                        Ok(event) => {
                            if dirty && let Event::Frame = event  {
                                debug!("drawing frame");
                                tui.draw(|frame| tea.view(&mut model, frame)).expect("terminal draw failed");
                                dirty = false;
                            }
                            let _  = event_tx.send(event);
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

    fn spawn_cmd<M: Msg>(cmd: Cmd<M>, action_tx: UnboundedSender<Action<M>>) -> JoinHandle<()> {
        tokio::spawn(async move {
            for cmd in cmd.0 {
                let action = cmd.await;
                action_tx
                    .send(action)
                    .expect("action/msg channel was closed unexpectedly");
            }
        })
    }

    fn rebuild_sub<M: Msg>(
        sub: Sub<M>,
        event_rx: &Receiver<Event>,
        sub_map: &mut StreamMap<u64, MsgStream<M>>,
    ) {
        let mut new = StreamMap::new();
        for factory in sub.0 {
            let mut hasher = Hasher::new();
            factory.hash(&mut hasher);
            let hash = hasher.finish();
            if let Some(stream) = sub_map.remove(&hash) {
                new.insert(hash, stream);
            } else {
                debug!(?hash, "creating new subscription stream");
                let event_stream = tokio_stream::StreamExt::filter_map(
                    tokio_stream::wrappers::BroadcastStream::new(event_rx.resubscribe()),
                    |event| match event {
                        Ok(event) => Some(event),
                        Err(err) => {
                            warn!(?err, "");
                            None
                        }
                    },
                )
                .boxed();
                new.insert(hash, factory.stream(event_stream));
            }
        }
        *sub_map = new;
    }
}

impl Default for Runner {
    fn default() -> Self {
        Self { frame_rate: 30.0 }
    }
}
