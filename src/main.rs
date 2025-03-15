mod choose;
mod confirm;
mod spinner;
mod text;
mod typer;

use std::{
    io::{stderr, stdin, Stderr},
    num::NonZeroUsize,
    process::{Command, ExitCode, Stdio},
    time::{Duration, Instant},
};

use choose::Choose;
use clap::{command, Parser, Subcommand};
use confirm::Confirm;
use crossterm::{
    cursor::{Hide, Show},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::Color,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lru::LruCache;
use spinner::Spinner;
use text::Text;
use typer::Typer;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Parser)]
#[command(name = "rum")]
#[command(bin_name = "rum")]
#[clap(disable_help_flag = true)]
struct Opts {
    /// Styling string
    #[arg(short, long)]
    style: Option<String>,

    /// Viewport height
    #[arg(short, long)]
    height: Option<usize>,

    /// Viewport width
    #[arg(short, long, default_value_t = 32)]
    width: usize,

    /// Subcommand
    #[structopt(subcommand)]
    subcommand: Cmd,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum SpinnerStyle {
    Braille,
    VBar,
    Arrow,
    Circle,
    Pulse,
    Line,
    Moon,
    Monkey,
    Meter,
    Points,
    Progress,
}

#[derive(Debug, Subcommand, Clone)]
enum Cmd {
    /// Single line text input
    Text {
        /// Placeholder text
        #[arg(short, long, default_value = "Enter text here")]
        placeholder: String,

        /// Prefix
        #[arg(short, long, default_value = "> ")]
        prefix: String,
    },
    /// Binary confirmation input
    Confirm {
        /// Title text
        #[arg(short, long, default_value = "Confirm?")]
        text: String,

        /// No option text
        #[arg(short, long, default_value = "No")]
        no: String,

        /// Yes option text
        #[arg(short, long, default_value = "Yes")]
        yes: String,
    },
    /// Spinner progress indicator
    Spinner {
        /// Text
        #[arg(short, long, default_value = "Waiting ...")]
        text: String,

        /// Spinner speed, milliseconds between frames
        #[arg(short = 'i', long, default_value = "100")]
        speed: usize,

        /// Spinner style
        #[arg(short, long, default_value = "braille")]
        spinner_style: SpinnerStyle,

        /// The subcommand to spawn a child process
        #[arg(name = "COMMAND", required = true)]
        command: Vec<String>,
    },
    /// Typing effect
    Typer {
        #[arg(short = 'i', long, default_value = "100")]
        speed: usize,
        #[arg(short, long, default_value = "1000")]
        wait: usize,
        #[arg(short, long)]
        text: String,
    },
    /// Choose from a few different options. Options are read in from stdin, separated by newlines.
    Choose {
        /// Number of allowed selections
        #[arg(short, long, default_value = "1")]
        selections: NonZeroUsize,

        /// Allow for fewer than requested selections
        #[arg(short, long)]
        inexact: bool,

        /// Text
        #[arg(short, long, default_value = "Choose from these options:")]
        text: String,
    },
}

trait DropError<V> {
    fn drop_error(self) -> Result<V, ()>;
}

impl<V, E> DropError<V> for Result<V, E> {
    fn drop_error(self) -> Result<V, ()> {
        self.map_err(|_| ())
    }
}

fn get_bg_color(active: bool) -> Color {
    if active {
        Color::Magenta
    } else {
        Color::DarkGrey
    }
}

#[enum_dispatch::enum_dispatch(ComponentTrait)]
enum Component {
    Text(Text),
    Confirm(Confirm),
    Spinner(Spinner),
    Typer(Typer),
    Choose(Choose),
}

#[enum_dispatch::enum_dispatch]
trait ComponentTrait {
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
            Cmd::Text {
                placeholder,
                prefix,
            } => Component::Text(Text {
                width: opts.width,
                placeholder,
                prefix,
                input: String::new(),
            }),
            Cmd::Confirm { text, no, yes } => Component::Confirm(Confirm {
                text: text.clone(),
                padded_no: format!(" {: ^10} ", no),
                padded_yes: format!(" {: ^10} ", yes),
                confirmed: false,
            }),
            Cmd::Spinner {
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
            Cmd::Typer { speed, text, wait } => Component::Typer(Typer {
                speed: Duration::from_millis(speed as u64),
                wait: Duration::from_millis(wait as u64),
                graphemes: text.graphemes(true).map(|s| s.to_owned()).rev().collect(),
                last_updated: Instant::now(),
                done_printing: false,
            }),
            Cmd::Choose {
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

fn main() -> Result<ExitCode, ()> {
    let opts = Opts::parse();

    // Create component
    let mut component = Component::from_opts(&opts);

    let mut screen = stderr();

    // enter the alternate screen
    execute!(screen, EnterAlternateScreen, Hide).drop_error()?;
    enable_raw_mode().drop_error()?;

    // Component setup.
    component.draw(&mut screen)?;
    let mut interrupted = false;

    // Component loop.
    loop {
        if component.tick(&mut screen)? {
            break;
        }

        // redraw
        component.draw(&mut screen)?;

        if !poll(Duration::from_millis(50)).unwrap() {
            continue;
        }

        let event = read().drop_error()?;

        // exit on control c
        if let Event::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) = event
        {
            interrupted = true;
            break;
        }
        if component.handle_event(&event, &mut screen)? {
            break;
        }
        // redraw
        component.draw(&mut screen)?;
    }
    disable_raw_mode().drop_error()?;
    execute!(screen, Show, LeaveAlternateScreen).drop_error()?;

    let res = if interrupted {
        Err(1u8)
    } else {
        component.result()
    };

    if let Ok(to_print) = &res {
        print!("{to_print}")
    }

    Ok(ExitCode::from(res.err().unwrap_or(0)))
}
