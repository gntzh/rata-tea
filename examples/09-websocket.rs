use rata_tea::{
    Application, Cmd, Sub,
    ratatui::{Action, Runner, on_term_event},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    text::{Line, Text},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_textarea::{Input, Key, TextArea};
mod common;

/// todo
fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::init, Model::update, Model::view)
        .subscriptions(Model::subscriptions);
    Runner::default().msg_to_action(Into::into).run(tea)?;
    Ok(())
}

enum Msg {
    DraftChanged(Input),
    /// send message to ws
    Send,
    /// receive message from ws
    Recv(ws::Msg),
    Quit,
}

impl From<Msg> for Action<Msg> {
    fn from(val: Msg) -> Self {
        match val {
            Msg::Quit => Action::Quit,
            _ => Action::Msg(val),
        }
    }
}

struct Model {
    draft: TextArea<'static>,
    messages: Vec<String>,
    ws: Option<ws::Sender>,
}

impl Model {
    fn init() -> (Self, Cmd<Msg>) {
        let mut textarea = ratatui_textarea::TextArea::default();
        textarea.set_block(Block::default().borders(Borders::ALL));
        let model = Self {
            draft: textarea,
            messages: Default::default(),
            ws: None,
        };
        (
            model,
            ws::create_ws("wss://echo.websocket.org").map(Msg::Recv),
        )
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Quit => unreachable!(),
            Msg::DraftChanged(input) => {
                self.draft.input(input);
            }
            Msg::Recv(ws::Msg::Connected(sender)) => {
                let _ = self.ws.replace(sender);
            }
            Msg::Recv(ws::Msg::Received(s)) => self.messages.push(s),
            Msg::Recv(ws::Msg::SendFailed(s)) => self.messages.push(s),
            Msg::Recv(ws::Msg::ConnectFailed) => {
                self.messages.push("WebSocket connect failed.".to_owned())
            }
            Msg::Recv(ws::Msg::Disconnected) => {
                self.messages.push("WebSocket was disconnected.".to_owned())
            }
            Msg::Send if let Some(sender) = &self.ws => {
                let sender = sender.clone();
                let s = self.draft.lines().join("");
                self.draft.clear();
                return Cmd::effect({
                    async move {
                        let _ = sender.send(ws::Action::Send(s)).await;
                    }
                });
            }
            Msg::Send => (),
        }
        Cmd::none()
    }

    fn subscriptions(&self) -> Sub<Msg> {
        on_term_event().filter_map(|event| match event.into() {
            Input { key: Key::Esc, .. }
            | Input {
                key: Key::Char('c'),
                ctrl: true,
                ..
            } => Some(Msg::Quit),
            Input {
                key: Key::Char('m'),
                ctrl: true,
                ..
            } => None,
            Input {
                key: Key::Enter, ..
            } => Some(Msg::Send),
            input => Some(Msg::DraftChanged(input)),
        })
    }

    fn view(&self, frame: &mut Frame) {
        let layout = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]);
        let [draft_area, messages_area] = frame.area().layout(&layout);
        frame.render_widget(&self.draft, draft_area);
        frame.render_widget(
            Paragraph::new(Text::from_iter(
                self.messages.iter().map(|s| Line::from(s.as_str())),
            )),
            messages_area,
        );
    }
}

pub mod ws {

    use futures::{SinkExt, StreamExt};
    use rata_tea::{BoxDispatch, Cmd};
    use tokio_tungstenite::tungstenite::{self};

    pub type Sender = tokio::sync::mpsc::Sender<Action>;
    pub enum Msg {
        Connected(Sender),
        ConnectFailed,
        Disconnected,
        SendFailed(String),
        Received(String),
    }

    pub enum Action {
        Disconnect,
        Send(String),
    }

    pub fn create_ws(url: impl AsRef<str> + Send + 'static) -> Cmd<Msg> {
        let (tx, mut rx) = tokio::sync::mpsc::channel::<Action>(100);
        let f = |dispatch: BoxDispatch<Msg>| async move {
            let mut websocket = match tokio_tungstenite::connect_async(url.as_ref()).await {
                Ok((wss, _)) => {
                    dispatch(Msg::Connected(tx)).await;
                    wss
                }
                Err(_) => {
                    dispatch(Msg::ConnectFailed).await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    received =  websocket.select_next_some() => {
                        match received {
                            Ok(tungstenite::Message::Text(s) ) => {
                                dispatch(Msg::Received(s.to_string())).await;
                            }
                            Ok(_) => {},
                            Err(_) => {
                                dispatch(Msg::Disconnected).await;
                                break;
                            }
                        }
                    }
                    Some(action) = rx.recv() => {
                        match action {
                            Action::Disconnect => {
                                let _ =  websocket.close(None).await;
                                break;
                            },
                            Action::Send(s) => {
                                if let Err(err) =  websocket.send(tungstenite::Message::Text(s.into())).await {
                                    dispatch(Msg::SendFailed(err.to_string())).await;
                                    dispatch(Msg::Disconnected).await;
                                    break;
                                };
                            },
                        }
                    }
                }
            }
        };
        Cmd::from_fn(f)
    }
}
