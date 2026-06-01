use std::collections::HashMap;

use futures::{FutureExt, StreamExt};
use tokio::task::JoinHandle;
use tracing::{error, trace, warn};

use crate::{Cmd, Dispatch, Sub, SubId, Tick, terminal::GLOBAL_EVENT_BUS};

pub struct Runner<Tea: crate::Tea> {
    frame_rate: f64,
    msg_to_action: fn(Tea::Msg) -> Action<Tea::Msg>,
}

pub enum Action<Msg> {
    Msg(Msg),
    Quit,
}

impl<Tea: crate::Tea> Runner<Tea>
where
    for<'a> Tea::View<'a>: FnOnce(&mut ratatui::Frame),
{
    pub fn msg_to_action(self, msg_to_action: fn(Tea::Msg) -> Action<Tea::Msg>) -> Self {
        Self {
            msg_to_action,
            ..self
        }
    }

    pub fn frame_rate(self, frame_rate: f64) -> Self {
        Self { frame_rate, ..self }
    }

    #[tokio::main]
    pub async fn run(self, tea: Tea) -> color_eyre::Result<()> {
        color_eyre::install()?;
        let mut tui = ratatui::try_init()?;

        let (msg_tx, mut msg_rx) = tokio::sync::mpsc::channel::<Tea::Msg>(1024);
        let dispatch = {
            move |msg| {
                let tx = msg_tx.clone();
                (async move {
                    match tx.send(msg).await {
                        Ok(()) => (),
                        Err(_) => error!("msg channel was closed unexpectedly"),
                    }
                })
                .boxed()
            }
        };
        let (mut model, cmd) = tea.init();
        let mut active_sub = HashMap::new();

        let mut ticker =
            tokio::time::interval(std::time::Duration::from_secs_f64(1.0 / self.frame_rate));
        let mut terminal_event_stream = crossterm::event::EventStream::new();

        Self::rebuild_sub(tea.subscriptions(&model), dispatch.clone(), &mut active_sub);
        Self::spawn_cmd(cmd, dispatch.clone());
        let mut dirty = true;

        loop {
            tokio::select! {
                msg = msg_rx.recv() => {
                    match msg {
                        Some(msg) => {
                            match (self.msg_to_action)(msg) {
                                Action::Msg(msg) => {
                                    let cmd = tea.update(&mut model, msg);
                                    dirty = true;
                                    Self::rebuild_sub(tea.subscriptions(&model), dispatch.clone(), &mut active_sub);
                                    Self::spawn_cmd(cmd, dispatch.clone());
                                },
                                Action::Quit => {
                                    break;
                                }
                            }
                        }
                        None => {
                            warn!("msg channel was closed unexpectedly")
                        }
                    }
                }
                _tick = ticker.tick() => {
                    if dirty {
                        trace!("drawing frame");
                        let view = tea.view(&mut model);
                        tui.draw(view).expect("terminal draw failed");
                        dirty = false;
                    }
                    GLOBAL_EVENT_BUS.publish(Tick).await;
                }
                Some(term_event) = terminal_event_stream.next() => {
                    match term_event {
                        Ok(term_event) => GLOBAL_EVENT_BUS.publish(term_event).await,
                        Err(err) => {
                            warn!(?err, "error reading terminal event");
                        }
                    }
            }};
        }

        ratatui::try_restore()?;
        Ok(())
    }

    fn spawn_cmd<Msg: Send + 'static>(
        cmd: Cmd<Msg>,
        dispatch: impl Dispatch<Msg> + Clone + 'static,
    ) {
        for cmd in cmd.0 {
            tokio::spawn(cmd.execute(Box::new(dispatch.clone())));
        }
    }

    fn rebuild_sub<Msg: Send + 'static>(
        sub: Sub<Msg>,
        dispatch: impl Dispatch<Msg> + Clone + 'static,
        active_sub: &mut HashMap<SubId, DropHandle>,
    ) {
        let mut new = HashMap::new();
        for (id, factory) in sub.0 {
            if let Some(stream) = active_sub.remove(&id) {
                new.insert(id, stream);
            } else {
                trace!(?id, "creating new subscription stream");
                let handle = tokio::spawn(factory.create(Box::new(dispatch.clone())));
                new.insert(id, DropHandle(handle));
            }
        }
        *active_sub = new;
    }
}

impl<Tea: crate::Tea> Default for Runner<Tea> {
    fn default() -> Self {
        Self {
            frame_rate: 30.0,
            msg_to_action: Action::Msg,
        }
    }
}

struct DropHandle(JoinHandle<()>);

impl Drop for DropHandle {
    fn drop(&mut self) {
        self.0.abort();
    }
}
