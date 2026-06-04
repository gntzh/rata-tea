use rata_tea::{
    Application, Cmd, Sub,
    runner::{Action, Runner, on_term_event},
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use ratatui_textarea::{Input, Key, TextArea};
mod common;

fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::new, Model::update, Model::view)
        .subscriptions(Model::subscriptions);
    Runner::default().msg_to_action(Msg::into_action).run(tea)?;
    Ok(())
}

enum Msg {
    Quit,
    Input(Input),
    SwitchNext,
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
    name: TextArea<'static>,
    password: TextArea<'static>,
    password_again: TextArea<'static>,
    active: Active,
}

pub enum Active {
    Name,
    Password,
    PasswordAgain,
}

impl Model {
    fn new() -> Self {
        let mut name = ratatui_textarea::TextArea::default();
        name.set_block(Block::default().borders(Borders::ALL));
        name.set_placeholder_text("Name");
        let mut password = ratatui_textarea::TextArea::default();
        password.set_block(Block::default().borders(Borders::ALL));
        password.set_placeholder_text("Password");

        let mut password_again = ratatui_textarea::TextArea::default();
        password_again.set_block(Block::default().borders(Borders::ALL));
        password_again.set_placeholder_text("Re-enter Password");

        password.set_mask_char('*');
        password_again.set_mask_char('*');

        let mut model = Self {
            name,
            password,
            password_again,
            active: Active::Name,
        };
        model.activate();
        model
    }

    fn subscriptions(&self) -> Sub<Msg> {
        on_term_event().filter_map(|event| match event.into() {
            Input { key: Key::Esc, .. }
            | Input {
                key: Key::Char('c'),
                ctrl: true,
                ..
            } => Some(Msg::Quit),
            Input { key: Key::Tab, .. } => Some(Msg::SwitchNext),
            Input {
                key: Key::Enter, ..
            }
            | Input {
                key: Key::Char('m'),
                ctrl: true,
                ..
            } => None,
            input => Some(Msg::Input(input)),
        })
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            // Handle input for each textarea
            Msg::Input(input) => {
                self.active().0.input(input);
            }
            Msg::SwitchNext => {
                self.active = match self.active {
                    Active::Name => Active::Password,
                    Active::Password => Active::PasswordAgain,
                    Active::PasswordAgain => Active::Name,
                };
                self.activate();
            }
            Msg::Quit => unreachable!(),
        }
        Cmd::none()
    }

    fn active(&mut self) -> (&mut TextArea<'static>, [&mut TextArea<'static>; 2]) {
        match self.active {
            Active::Name => (
                &mut self.name,
                [&mut self.password, &mut self.password_again],
            ),
            Active::Password => (
                &mut self.password,
                [&mut self.name, &mut self.password_again],
            ),
            Active::PasswordAgain => (
                &mut self.password_again,
                [&mut self.name, &mut self.password],
            ),
        }
    }
    fn activate(&mut self) {
        let (active, not_actives) = self.active();
        active.set_cursor_line_style(Style::default().add_modifier(Modifier::UNDERLINED));
        active.set_cursor_style(Style::default().add_modifier(Modifier::REVERSED));
        for not_active in not_actives {
            not_active.set_cursor_line_style(Style::default());
            not_active.set_cursor_style(Style::default());
        }
    }

    fn view(&self, frame: &mut Frame) {
        let [help_area, form_area, msg_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .areas(frame.area());

        frame.render_widget(Paragraph::new("<Tab> to switch, <Esc> to quit."), help_area);

        let [name_area, password_area, password_again_area] =
            Layout::horizontal(Constraint::from_fills([1, 1, 1])).areas(form_area);
        frame.render_widget(&self.name, name_area);
        frame.render_widget(&self.password, password_area);
        frame.render_widget(&self.password_again, password_again_area);
        let text = if self.password.lines() == self.password_again.lines() {
            Line::raw("OK").style(Style::new().green())
        } else {
            Line::raw("Passwords do not match!").style(Style::new().red())
        };
        frame.render_widget(Paragraph::new(text), msg_area);
    }
}
