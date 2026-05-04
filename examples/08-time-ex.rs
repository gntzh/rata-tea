use std::time::Duration;

use chrono::Local;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    style::Style,
    text::{Line, Text, ToSpan},
    widgets::Paragraph,
};
use ratatui_tea_examples::{Application, Cmd, Sub, term::on_key_press, time::every};
mod common;

fn main() -> color_eyre::Result<()> {
    common::initialize_logging()?;
    let tea = Application::new(Model::new, Model::update, Model::view)
        .with_subscriptions(Model::subscriptions);
    ratatui_tea_examples::Runner::default().run(tea)?;
    Ok(())
}

struct Model {
    time: chrono::DateTime<Local>,
    run: bool,
}

enum Msg {
    Tick,
    Switch,
    Quit,
}

impl Model {
    fn new() -> Self {
        Self {
            time: Local::now(),
            run: true,
        }
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
            } => Some(Msg::Quit),
            event => match event.code {
                KeyCode::Char('p') => Some(Msg::Switch),
                _ => None,
            },
        })];
        if self.run {
            subs.push(every(Duration::from_millis(1000)).map(|_| Msg::Tick))
        }
        Sub::batch(subs)
    }

    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Tick => self.time = Local::now(),
            Msg::Switch => self.run = !self.run,
            Msg::Quit => return Cmd::quit(),
        }
        Cmd::none()
    }

    fn view(&mut self, frame: &mut Frame) {
        frame.render_widget(
            Paragraph::new(Text::from(vec![
                Line::from(format!("<p> to {}", if self.run { "pause" } else { "run" })),
                Line::from(vec![
                    self.time.format("%H:%M:").to_span(),
                    self.time.format("%S").to_span().style(Style::new().red()),
                ]),
            ])),
            frame.area(),
        );
    }
}
