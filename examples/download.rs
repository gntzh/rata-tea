use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rata_tea::{Application, Cmd, Runner, Sub, terminal::on_key_press};
use ratatui::{Frame, widgets::Paragraph};
mod common;

fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::new, Model::update, Model::view)
        .subscriptions(Model::subscriptions);
    Runner::default().run(tea)?;
    Ok(())
}

enum Msg {
    Increment,
    Decrement,
    Quit,
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

    fn view(&self, frame: &mut Frame) {
        frame.render_widget(
            Paragraph::new(format!("Count: {}", self.count)),
            frame.area(),
        );
    }
}
