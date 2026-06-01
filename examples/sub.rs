use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rata_tea::{
    Action, Application, Cmd, Runner, Sub,
    terminal::{on_key_press, on_term_event},
};
use ratatui::{Frame, widgets::Paragraph};
mod common;

fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::new, Model::update, Model::view)
        .subscriptions(Model::subscriptions);
    Runner::default().msg_to_action(Msg::into_action).run(tea)?;
    Ok(())
}

enum Msg {
    Increment,
    Decrement,
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
        let mut subs = vec![on_key_press().filter_map(|event| match event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('q'),
                ..
            }
            | KeyEvent {
                code: KeyCode::Esc, ..
            } => Some(Msg::Quit),
            event => match event.code {
                KeyCode::Char('+') => Some(Msg::Increment),
                KeyCode::Char('-') => Some(Msg::Decrement),
                _ => None,
            },
        })];
        if [3, 4, 5].contains(&(self.count % 10)) {
            subs.push(on_term_event().filter_map(|_| None));
        }
        Sub::batch(subs)
    }

    fn view(&mut self) -> impl FnOnce(&mut Frame) {
        move |frame| {
            frame.render_widget(
                Paragraph::new(format!("Count: {}", self.count)),
                frame.area(),
            );
        }
    }
}
