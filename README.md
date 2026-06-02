# rata-tea

The Elm Architecture (TEA) for [ratatui](https://github.com/ratatui-org/ratatui).

`rata-tea` helps structure terminal applications around four pieces:

- `Model`: your application state.
- `Msg`: events that can change the model.
- `Cmd`: one-shot asynchronous work that can dispatch messages.
- `Sub`: long-running subscriptions such as keyboard, timer, or network events.

The crate is currently `0.1.x`, so the public API is intended to be useful but may still evolve.

## Installation

```toml
[dependencies]
rata-tea = "0.1"
```

## Minimal Example

```rust
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

    fn view(&self, frame: &mut Frame) {
        frame.render_widget(
            Paragraph::new(format!("Count: {}", self.count)),
            frame.area(),
        );
    }
}
```

Run the repository examples with:

```sh
cargo run --example 01-buttons
```

## Examples

- `01-buttons`: keyboard-driven counter.
- `02-text_fields`: text input.
- `03-forms`: form state.
- `05-http`: asynchronous HTTP command.
- `08-time-ex`: timer subscription.
- `09-websocket`: websocket command/subscription sketch.

## Development

Before opening a PR, run:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo doc --no-deps
cargo package
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution and release guidance.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
