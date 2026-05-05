use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Frame, widgets::Paragraph};
use ratatui_tea_examples::{Action, Application, Cmd, Runner, Sub, terminal::on_key_press};
mod common;

/// todo
fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::new, Model::update, Model::view)
        .subscriptions(Model::subscriptions);
    Runner::default().msg_to_action(Into::into).run(tea)?;
    Ok(())
}

enum Msg {
    Increment,
    Decrement,
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
    count: u8,
}

impl Model {
    fn new() -> Self {
        Self { count: 0 }
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Quit => unreachable!(),
            Msg::Increment => self.count = self.count.saturating_add(1),
            Msg::Decrement => self.count = self.count.saturating_sub(1),
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
            event => match event.code {
                KeyCode::Char('+') => Some(Msg::Increment),
                KeyCode::Char('-') => Some(Msg::Decrement),
                _ => None,
            },
        })
    }

    fn view(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Paragraph::new(format!("Count: {}", self.count)),
            frame.area(),
        );
    }
}

async fn create_ws(url: String) {
    let a = tokio_tungstenite::connect_async(url).await;
}
