use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    widgets::{Paragraph, Wrap},
};
use ratatui_tea_examples::{Action, Application, Cmd, Runner, Sub, terminal::on_key_press};
use tracing::debug;
mod common;

fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::init, Model::update, Model::view)
        .subscriptions(Model::subscriptions);
    Runner::default().msg_to_action(Msg::into_action).run(tea)?;
    Ok(())
}

enum Msg {
    GoText(reqwest::Result<String>),
    ScrollUp,
    ScrollDown,
    Quit,
}

impl Msg {
    fn into_action(self) -> Action<Msg> {
        match self {
            Msg::Quit => Action::Quit,
            _ => Action::Msg(self),
        }
    }
}

enum Model {
    Failure,
    Loading,
    Success((String, u16)),
}

impl Model {
    fn init() -> (Self, Cmd<Msg>) {
        (
            Self::Loading,
            Cmd::from_fn(|dispatch| async move {
                let res = match reqwest::get("https://elm-lang.org/assets/public-opinion.txt").await
                {
                    Ok(resp) => resp.text().await,
                    Err(err) => Err(err),
                };
                dispatch(Msg::GoText(res)).await;
            }),
        )
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Quit => unreachable!(),
            Msg::GoText(Ok(text)) => *self = Self::Success((text, 0)),
            Msg::GoText(Err(_)) => *self = Self::Failure,
            Msg::ScrollUp => {
                if let Self::Success((_, scroll)) = self {
                    *scroll = scroll.saturating_sub(1);
                }
            }
            Msg::ScrollDown => {
                if let Self::Success((_, scroll)) = self {
                    *scroll = scroll.saturating_add(1);
                }
            }
        }
        Cmd::none()
    }

    fn subscriptions(&self) -> Sub<Msg> {
        on_key_press().filter_map(|event| match event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('q'),
                ..
            } => Some(Msg::Quit),
            KeyEvent {
                code: KeyCode::Char('j'),
                ..
            } => Some(Msg::ScrollDown),
            KeyEvent {
                code: KeyCode::Char('k'),
                ..
            } => Some(Msg::ScrollUp),
            _ => None,
        })
    }

    fn view(&mut self, frame: &mut Frame) {
        let (text, scroll) = match self {
            Model::Loading => ("Loading...", 0),
            Model::Failure => ("I was unable to load your book.", 0),
            Model::Success((s, scroll)) => (s.as_str(), *scroll),
        };
        debug!(scroll);
        frame.render_widget(
            Paragraph::new(text)
                .scroll((scroll, 0))
                .wrap(Wrap::default()),
            frame.area(),
        );
    }
}
