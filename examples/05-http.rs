use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rata_tea::{Action, Application, Cmd, Runner, Sub, terminal::on_key_press};
use ratatui::{
    Frame,
    widgets::{Paragraph, Wrap},
};
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
    Document(book::Msg),
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
    Success(book::Model),
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
            Msg::GoText(Ok(text)) => *self = Self::Success(book::Model::new(text)),
            Msg::GoText(Err(_)) => *self = Self::Failure,
            Msg::Document(msg) => {
                if let Self::Success(book) = self {
                    return book.update(msg).map(Msg::Document);
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
                code: KeyCode::Char('j') | KeyCode::Down,
                ..
            } => Some(Msg::Document(book::Msg::ScrollDown)),
            KeyEvent {
                code: KeyCode::Char('k') | KeyCode::Up,
                ..
            } => Some(Msg::Document(book::Msg::ScrollUp)),
            KeyEvent {
                code: KeyCode::PageDown,
                ..
            } => Some(Msg::Document(book::Msg::PageDown)),
            KeyEvent {
                code: KeyCode::PageUp,
                ..
            } => Some(Msg::Document(book::Msg::PageUp)),
            _ => None,
        })
    }

    fn view(&mut self, frame: &mut Frame) {
        match self {
            Self::Loading => frame.render_widget(
                Paragraph::new("Loading...").wrap(Wrap::default()),
                frame.area(),
            ),
            Self::Failure => frame.render_widget(
                Paragraph::new("I was unable to load your book.").wrap(Wrap::default()),
                frame.area(),
            ),
            Self::Success(book) => book.view(frame),
        }
    }
}

mod book {
    use std::ops::Range;

    use rata_tea::Cmd;
    use ratatui::{
        Frame,
        text::{Line, Text},
        widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    };

    pub struct Model {
        text: String,
        lines: Vec<Range<usize>>,
        line_count: usize,
        viewport_height: u16,
        scrollbar: ScrollbarState,
    }
    pub enum Msg {
        ScrollUp,
        ScrollDown,
        PageUp,
        PageDown,
    }

    impl Model {
        pub fn new(text: String) -> Self {
            let lines = line_ranges(&text);
            let line_count = lines.len();
            Self {
                text,
                lines,
                line_count,
                viewport_height: 0,
                scrollbar: ScrollbarState::new(line_count),
            }
        }

        pub fn update(&mut self, msg: Msg) -> Cmd<Msg> {
            match msg {
                Msg::ScrollUp => self.scroll_up(),
                Msg::ScrollDown => self.scroll_down(),
                Msg::PageUp => self.page_up(),
                Msg::PageDown => self.page_down(),
            }
            Cmd::none()
        }

        pub fn view(&mut self, frame: &mut Frame) {
            let area = frame.area();
            self.viewport_height = area.height;
            self.scrollbar = self.scrollbar.viewport_content_length(area.height as usize);
            let scroll_position = self.scrollbar.get_position();
            let visible_text = Text::from_iter(
                self.lines
                    .iter()
                    .skip(scroll_position)
                    .take(area.height as usize)
                    .map(|line| Line::raw(&self.text[line.clone()])),
            );
            frame.render_widget(Paragraph::new(visible_text), area);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                area,
                &mut self.scrollbar,
            );
        }
    }

    fn line_ranges(text: &str) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        let mut start = 0;

        for segment in text.split_inclusive('\n') {
            let end = start + segment.len();
            let mut line_end = end;
            if segment.ends_with('\n') {
                line_end -= 1;
                if line_end > start && text.as_bytes()[line_end - 1] == b'\r' {
                    line_end -= 1;
                }
            }
            ranges.push(start..line_end);
            start = end;
        }

        ranges
    }

    impl Model {
        fn max_scroll_position(&self) -> usize {
            self.line_count
                .saturating_sub(self.viewport_height as usize)
        }

        fn scroll_up(&mut self) {
            self.scrollbar.prev();
        }
        fn scroll_down(&mut self) {
            if self.scrollbar.get_position() < self.max_scroll_position() {
                self.scrollbar.next();
            }
        }
        fn page_up(&mut self) {
            let position = self
                .scrollbar
                .get_position()
                .saturating_sub(self.viewport_height as usize);
            self.scrollbar = self.scrollbar.position(position);
        }
        fn page_down(&mut self) {
            let position = self
                .scrollbar
                .get_position()
                .saturating_add(self.viewport_height as usize)
                .min(self.max_scroll_position());
            self.scrollbar = self.scrollbar.position(position);
        }
    }
}
