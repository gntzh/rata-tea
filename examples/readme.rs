use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rata_tea::{Action, Application, Cmd, Runner, Sub, terminal::on_key_press};
use ratatui::{Frame, widgets::Paragraph};

fn main() -> color_eyre::Result<()> {
    let app = Application::new(Model::new, Model::update, Model::view)
        .subscriptions(Model::subscriptions);

    Runner::default().msg_to_action(Msg::into_action).run(app)
}

enum Msg {
    Increment,
    Decrement,
    Quit,
}

impl Msg {
    fn into_action(self) -> Action<Self> {
        match self {
            Self::Quit => Action::Quit,
            msg => Action::Msg(msg),
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
            Msg::Increment => self.count = self.count.saturating_add(1),
            Msg::Decrement => self.count = self.count.saturating_sub(1),
            Msg::Quit => {}
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

    fn view(&mut self) -> impl FnOnce(&mut Frame) {
        move |frame| {
            frame.render_widget(
                Paragraph::new(format!("Count: {}", self.count)),
                frame.area(),
            );
        }
    }
}
