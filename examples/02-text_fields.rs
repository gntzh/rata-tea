use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    widgets::{Block, Borders, Paragraph},
};
use ratatui_tea_examples::{Application, Cmd, Sub, term::on_term_event};
use ratatui_textarea::{Input, Key};
mod common;

fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::new, Model::update, Model::view)
        .with_subscriptions(Model::subscriptions);
    ratatui_tea_examples::Runner::default().run(tea)?;
    Ok(())
}

enum Msg {
    Quit,
    Input(Input),
}

struct Model {
    textarea: ratatui_textarea::TextArea<'static>,
}

impl Model {
    fn new() -> Self {
        let mut textarea = ratatui_textarea::TextArea::default();
        textarea.set_block(Block::default().borders(Borders::ALL));
        Self { textarea }
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Input(input) => {
                self.textarea.input(input);
            }
            Msg::Quit => return Cmd::quit(),
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

    fn view(&mut self, frame: &mut Frame) {
        let layout = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]);
        let [input_area, display_area] = frame.area().layout(&layout);
        frame.render_widget(&self.textarea, input_area);
        frame.render_widget(
            Paragraph::new(self.textarea.lines()[0].to_owned()),
            display_area,
        );
    }
}
