use std::{
    io::{stdin, Stderr},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use crossterm::event::Event;
use lru::LruCache;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{
    choose::Choose, confirm::Confirm, spinner::Spinner, text::Text, typer::Typer, CommandOpt, Opts,
    SpinnerStyle,
};

#[enum_dispatch::enum_dispatch(ComponentTrait)]
pub enum Component {
    Text(Text),
    Confirm(Confirm),
    Spinner(Spinner),
    Typer(Typer),
    Choose(Choose),
}

#[enum_dispatch::enum_dispatch]
pub trait ComponentTrait {
    /// Return the result and return code
    fn result(self) -> Result<String, u8>;

    /// Tick the component. Return Ok(true) if the component is complete.
    fn tick(&mut self, _screen: &mut Stderr) -> Result<bool, ()> {
        Ok(false)
    }

    /// Process a terminal event. Return Ok(true) if the component is complete.
    fn handle_event(&mut self, event: &Event, screen: &mut Stderr) -> Result<bool, ()>;

    /// Draw the component
    fn draw(&mut self, screen: &mut Stderr) -> Result<(), ()>;
}

impl Component {
    pub fn from_opts(opts: &Opts) -> Component {
        match opts.subcommand.clone() {
            CommandOpt::Text {
                placeholder,
                prefix,
            } => Component::Text(Text {
                width: opts.width,
                placeholder,
                prefix,
                input: String::new(),
            }),
            CommandOpt::Confirm { text, no, yes } => Component::Confirm(Confirm {
                text: text.clone(),
                padded_no: format!(" {: ^10} ", no),
                padded_yes: format!(" {: ^10} ", yes),
                confirmed: false,
            }),
            CommandOpt::Spinner {
                text,
                speed,
                command,
                spinner_style,
            } => {
                let chars: Vec<String> = match spinner_style {
                    SpinnerStyle::Braille => vec!["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"],
                    SpinnerStyle::VBar => vec![
                        "▁", "▂", "▃", "▄", "▅", "▆", "▇", "█", "▇", "▆", "▅", "▄", "▃", "▂", "▁",
                    ],
                    SpinnerStyle::Arrow => vec!["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"],
                    SpinnerStyle::Circle => vec!["◜", "◠", "◝", "◞", "◡", "◟"],
                    SpinnerStyle::Pulse => vec!["█", "▓", "▒", "░"],
                    SpinnerStyle::Line => vec!["|", "/", "-", "\\"],
                    SpinnerStyle::Moon => vec![
                        "\u{1f311}",
                        "\u{1f312}",
                        "\u{1f313}",
                        "\u{1f314}",
                        "\u{1f315}",
                        "\u{1f316}",
                        "\u{1f317}",
                        "\u{1f318}",
                    ],
                    SpinnerStyle::Monkey => vec!["\u{1f648}", "\u{1f649}", "\u{1f64a}"],
                    SpinnerStyle::Meter => vec!["▱▱▱", "▰▱▱", "▰▰▱", "▰▰▰", "▰▰▱", "▰▱▱", "▱▱▱"],
                    SpinnerStyle::Points => vec!["∙∙∙", "●∙∙", "∙●∙", "∙∙●"],
                    SpinnerStyle::Progress => vec![
                        "[     ]", "[>    ]", "[=>   ]", "[==>  ]", "[===> ]", "[====>]", "[=====]",
                    ],
                }
                .iter()
                .map(ToString::to_string)
                .collect();

                let child = Command::new(&command[0])
                    .args(&command[1..])
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap();
                Component::Spinner(Spinner {
                    text,
                    chars,
                    last_updated: Instant::now(),
                    progress: 0,
                    child,
                    speed: Duration::from_millis(speed as u64),
                })
            }
            CommandOpt::Typer { speed, text, wait } => Component::Typer(Typer {
                speed: Duration::from_millis(speed as u64),
                wait: Duration::from_millis(wait as u64),
                graphemes: text.graphemes(true).map(|s| s.to_owned()).rev().collect(),
                last_updated: Instant::now(),
                done_printing: false,
            }),
            CommandOpt::Choose {
                selections,
                text,
                inexact,
            } => {
                // Grab all options from stdin
                let mut choices: Vec<String> = vec![];
                for line in stdin().lines() {
                    choices.push(line.unwrap());
                }
                if choices.is_empty() {
                    panic!("Got 0 options!");
                }

                let (selected_string, unselected_string) = if selections.get() == 1 {
                    ("(x) ".to_owned(), "( ) ".to_owned())
                } else {
                    ("[x] ".to_owned(), "[ ] ".to_owned())
                };
                Component::Choose(Choose {
                    text,
                    choices,
                    chosen: LruCache::new(selections),
                    cursor_loc: 0,
                    selections,
                    inexact,
                    selected_string,
                    unselected_string,
                })
            }
        }
    }
}
