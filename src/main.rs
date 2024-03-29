use std::{
    io::{stderr, stdin, Stderr},
    num::NonZeroUsize,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{
        Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
    },
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
};
use lru::LruCache;
use structopt::{clap::arg_enum, StructOpt};
use unicode_segmentation::{Graphemes, UnicodeSegmentation};

#[derive(Debug, StructOpt)]
#[structopt(name = "rum", about = "Stylish interactive scripts")]
struct Opts {
    /// Styling string
    #[structopt(short("s"), long)]
    style: Option<String>,

    /// Viewport height
    #[structopt(short("h"), long)]
    height: Option<usize>,

    /// Viewport width
    #[structopt(short("w"), long, default_value = "32")]
    width: usize,

    /// Subcommand
    #[structopt(subcommand)]
    subcommand: Subcommand,
}

arg_enum! {
    #[derive(Debug)]
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
}

#[derive(Debug, StructOpt)]
enum Subcommand {
    /// Single line text input
    #[structopt()]
    Text {
        /// Placeholder text
        #[structopt(short("p"), long, default_value = "Enter text here")]
        placeholder: String,

        /// Prefix
        #[structopt(short("x"), long, default_value = "> ")]
        prefix: String,
    },
    /// Binary confirmation input
    #[structopt()]
    Confirm {
        /// Title text
        #[structopt(short("t"), long, default_value = "Confirm?")]
        text: String,

        /// No option text
        #[structopt(short("n"), long, default_value = "No")]
        no: String,

        /// Yes option text
        #[structopt(short("y"), long, default_value = "Yes")]
        yes: String,
    },
    /// Spinner progress indicator
    #[structopt()]
    Spinner {
        /// Text
        #[structopt(short("t"), long, default_value = "Waiting ...")]
        text: String,

        /// Spinner speed, milliseconds between frames
        #[structopt(short("i"), long, default_value = "100")]
        speed: usize,

        /// Spinner style
        #[structopt(short("s"), long, possible_values = &SpinnerStyle::variants(), case_insensitive = true, default_value = "braille")]
        spinner_style: SpinnerStyle,

        /// The subcommand to spawn a child process
        #[structopt(name = "COMMAND", required = true)]
        command: Vec<String>,
    },
    /// Typing effect
    #[structopt()]
    Typer {
        #[structopt(short("i"), long, default_value = "100")]
        speed: usize,
        #[structopt(short("w"), long, default_value = "1000")]
        wait: usize,
        #[structopt(short("t"), long)]
        text: String,
    },
    /// Choose from a few different options
    #[structopt()]
    Choose {
        /// Number of allowed selections
        #[structopt(short("s"), long, default_value = "1")]
        selections: NonZeroUsize,

        /// Allow for fewer than requested selections
        #[structopt(short("i"), long)]
        inexact: bool,

        /// Text
        #[structopt(short("t"), long, default_value = "Choose from these options:")]
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

#[derive(Debug, Default)]
struct TextState {
    input: String,
}

#[derive(Debug, Default)]
struct ConfirmState {
    confirmed: bool,
}

#[derive(Debug)]
struct SpinnerState {
    child: Child,
    chars: Vec<String>,
    progress: usize,
    last_updated: Instant,
}

#[derive(Debug)]
struct ChooseState {
    choices: Vec<String>,
    chosen: LruCache<usize, ()>,
    selections: NonZeroUsize,
    cursor_loc: usize,
}

#[derive(Debug)]
struct TyperState<'a> {
    iter: Graphemes<'a>,
    done_printing: bool,
    last_updated: Instant,
}

enum Component<'a> {
    Text {
        width: usize,
        placeholder: String,
        prefix: String,
        state: TextState,
    },
    Confirm {
        text: String,
        padded_no: String,
        padded_yes: String,
        state: ConfirmState,
    },
    Spinner {
        speed: Duration,
        text: String,
        state: SpinnerState,
    },
    Typer {
        speed: Duration,
        wait: Duration,
        text: String,
        state: TyperState<'a>,
    },
    Choose {
        text: String,
        selected_string: String,
        unselected_string: String,
        inexact: bool,
        state: ChooseState,
    },
}

impl<'a> Component<'a> {
    pub fn from_opts(opts: &Opts) -> Component {
        match &opts.subcommand {
            Subcommand::Text {
                placeholder,
                prefix,
            } => Component::Text {
                width: opts.width,
                placeholder: placeholder.clone(),
                prefix: prefix.clone(),
                state: TextState::default(),
            },
            Subcommand::Confirm { text, no, yes } => {
                let no = no.clone();
                let yes = yes.clone();
                let padded_no = format!("{: ^10}", no);
                let padded_yes = format!("{: ^10}", yes);

                Component::Confirm {
                    text: text.clone(),
                    padded_no,
                    padded_yes,
                    state: ConfirmState::default(),
                }
            }
            Subcommand::Spinner {
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
                .map(|e| e.to_string())
                .collect();

                let child = Command::new(&command[0])
                    .args(&command[1..])
                    .stdout(Stdio::null())
                    .spawn()
                    .unwrap();
                Component::Spinner {
                    text: text.clone(),
                    state: SpinnerState {
                        chars: chars.to_owned(),
                        last_updated: Instant::now(),
                        progress: 0,
                        child,
                    },
                    speed: Duration::from_millis(*speed as u64),
                }
            }
            Subcommand::Typer { speed, text, wait } => Component::Typer {
                speed: Duration::from_millis(*speed as u64),
                wait: Duration::from_millis(*wait as u64),
                text: text.clone(),
                state: TyperState {
                    iter: text.graphemes(true),
                    last_updated: Instant::now(),
                    done_printing: false,
                },
            },
            Subcommand::Choose {
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
                    panic!("Got 0 choices!");
                }

                let (selected_string, unselected_string) = if selections.get() == 1 {
                    ("(x) ".to_owned(), "( ) ".to_owned())
                } else {
                    ("[x] ".to_owned(), "[ ] ".to_owned())
                };
                Component::Choose {
                    text: text.clone(),
                    state: ChooseState {
                        choices,
                        chosen: LruCache::new(*selections),
                        cursor_loc: 0,
                        selections: *selections,
                    },
                    inexact: *inexact,
                    selected_string,
                    unselected_string,
                }
            }
        }
    }

    /// Return the stdout and return code of the component
    pub fn result(self) -> Result<(String, u8), ()> {
        match self {
            Component::Text {
                state: TextState { input },
                ..
            } => Ok((input, 0)),
            Component::Confirm {
                state: ConfirmState { confirmed },
                ..
            } => Ok((String::new(), if confirmed { 0 } else { 1 })),
            Component::Spinner {
                state: SpinnerState { mut child, .. },
                ..
            } => {
                // Assume that child is already finished
                let output = child.try_wait().drop_error()?;
                if let Some(code) = output {
                    Ok(("".to_owned(), code.code().unwrap_or(1) as u8))
                } else {
                    child.kill().ok(); // swallow error
                    Ok(("".to_owned(), 1))
                }
            }
            Component::Typer { .. } => Ok((String::new(), 0)),
            Component::Choose {
                state: ChooseState {
                    choices, chosen, ..
                },
                ..
            } => {
                let s = chosen
                    .iter()
                    .filter_map(|(k, _)| choices.get(*k).map(ToOwned::to_owned))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok((s, 0))
            }
        }
    }

    pub fn tick(&mut self, screen: &mut Stderr) -> Result<bool, ()> {
        let should_redraw: bool = match self {
            Component::Spinner { state, speed, .. } => {
                if let Some(_) = state.child.try_wait().drop_error()? {
                    return Ok(true);
                }

                if state.last_updated.elapsed() > *speed {
                    // Update progress
                    state.progress = (state.progress + 1) % state.chars.len();
                    state.last_updated = Instant::now();
                    true
                } else {
                    false
                }
            }
            Component::Typer {
                state, speed, wait, ..
            } => {
                if state.done_printing {
                    if state.last_updated.elapsed() > *wait {
                        return Ok(true);
                    }
                } else {
                    if state.last_updated.elapsed() > *speed {
                        let c = state.iter.next();
                        if let Some(c) = c {
                            execute!(screen, Print(c)).drop_error()?;
                            state.last_updated = Instant::now();
                        } else {
                            state.done_printing = true;
                        }
                    }
                }
                false
            }
            _ => false,
        };

        if should_redraw {
            self.draw(screen)?;
        }

        Ok(false)
    }

    /// Update the component with keystroke event
    /// Returns Ok(true) if component is in the terminal state
    /// # Errors if unable to draw to the terminal
    pub fn update(&mut self, event: &Event, screen: &mut Stderr) -> Result<bool, ()> {
        let should_redraw: bool = match self {
            Component::Text {
                state: TextState { input },
                ..
            } => match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Char(c),
                    ..
                }) => {
                    input.push(*c);
                    true
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Backspace,
                    ..
                }) => {
                    input.pop();
                    true
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                }) => return Ok(true),
                _ => false,
            },
            Component::Confirm { state, .. } => match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Right,
                    ..
                }) => {
                    state.confirmed = true;
                    true
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Left,
                    ..
                }) => {
                    state.confirmed = false;
                    true
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: KeyModifiers::NONE,
                    ..
                }) => return Ok(true),
                _ => false,
            },
            Component::Spinner { .. } => false,
            Component::Typer { .. } => false,
            Component::Choose { inexact, state, .. } => match event {
                Event::Key(KeyEvent {
                    code: KeyCode::Down,
                    ..
                }) => {
                    if state.cursor_loc != state.choices.len() - 1 {
                        state.cursor_loc += 1;
                        true
                    } else {
                        false
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Up, ..
                }) => {
                    if state.cursor_loc != 0 {
                        state.cursor_loc -= 1;
                        true
                    } else {
                        false
                    }
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Char(' '),
                    ..
                }) => {
                    let curstate = state.chosen.get(&state.cursor_loc).is_some();
                    if curstate {
                        // Remove from selection
                        state.chosen.pop(&state.cursor_loc);
                    } else {
                        // Add to selection
                        state.chosen.push(state.cursor_loc, ());
                    }
                    true
                }
                Event::Key(KeyEvent {
                    code: KeyCode::Enter,
                    ..
                }) => {
                    if *inexact || state.chosen.len() == state.selections.get() {
                        return Ok(true);
                    }
                    false
                }
                _ => false,
            },
        };

        // for now, always redraw
        if should_redraw {
            self.draw(screen)?;
        }

        Ok(false)
    }

    pub fn draw(&mut self, screen: &mut Stderr) -> Result<(), ()> {
        // TODO: Use styling
        let padding = 2;
        execute!(screen, Clear(ClearType::All), MoveTo(padding, padding)).drop_error()?;

        match self {
            Component::Text {
                width,
                placeholder,
                prefix,
                state: TextState { input },
            } => {
                execute!(screen, MoveTo(padding, padding)).drop_error()?;

                let (is_bg, to_print) = match input.as_str() {
                    "" => {
                        // show first n graphemes of placeholder
                        let end = placeholder
                            .grapheme_indices(true)
                            .nth(*width)
                            .map(|(i, _)| i)
                            .unwrap_or(placeholder.len());
                        (true, &placeholder[..end])
                    }
                    s => {
                        // show last n graphemes of input
                        let start = s
                            .grapheme_indices(true)
                            .rev()
                            .nth(*width - 1)
                            .map(|(i, _)| i)
                            .unwrap_or(0);
                        (false, &s[start..])
                    }
                };

                // set style
                if is_bg {
                    execute!(
                        screen,
                        SetAttribute(Attribute::Italic),
                        SetAttribute(Attribute::Dim)
                    )
                    .drop_error()?;
                }

                execute!(
                    screen,
                    Print(prefix),
                    Print(to_print),
                    SetAttribute(Attribute::Reset)
                )
                .drop_error()?;

                Ok(())
            }
            Component::Confirm {
                text,
                padded_no,
                padded_yes,
                state: ConfirmState { confirmed },
            } => {
                // TODO: Truncate/wrap text
                execute!(
                    screen,
                    MoveTo(padding, padding),
                    Print(text),
                    MoveTo(padding, padding + 2),
                    SetBackgroundColor(get_bg_color(!*confirmed)),
                    Print(padded_no),
                    ResetColor,
                    Print("  "),
                    SetBackgroundColor(get_bg_color(*confirmed)),
                    Print(padded_yes),
                    ResetColor
                )
                .drop_error()?;

                Ok(())
            }
            Component::Spinner {
                text,
                state: SpinnerState {
                    chars, progress, ..
                },
                ..
            } => {
                let c = &chars[*progress];

                execute!(
                    screen,
                    MoveTo(padding, padding),
                    Print(format!("{c}  {text}")),
                )
                .drop_error()?;

                Ok(())
            }
            Component::Typer { .. } => Ok(()),
            Component::Choose {
                text,
                state,
                selected_string,
                unselected_string,
                inexact,
            } => {
                let mut line = padding;
                execute!(
                    screen,
                    MoveTo(padding, line),
                    Print(text),
                    MoveTo(padding, line + 1),
                    SetAttribute(Attribute::Dim),
                    SetAttribute(Attribute::Italic),
                    Print(format!(
                        "Select {} {}",
                        if *inexact { "at most" } else { "exactly" },
                        state.selections.get()
                    )),
                    SetAttribute(Attribute::Reset)
                )
                .drop_error()?;

                line += 3;
                for (choice_i, choice) in state.choices.iter().enumerate() {
                    if choice_i == state.cursor_loc {
                        execute!(screen, SetForegroundColor(get_bg_color(true))).drop_error()?;
                    }

                    let selection: &str = if state.chosen.contains(&choice_i) {
                        &selected_string
                    } else {
                        &unselected_string
                    };

                    execute!(
                        screen,
                        MoveTo(padding, line),
                        Print(format!("{selection} {choice}")),
                        ResetColor
                    )
                    .drop_error()?;

                    line += 1;
                }

                Ok(())
            }
        }
    }
}

fn main() -> Result<(), ()> {
    let opts = Opts::from_args();

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
        if component.update(&event, &mut screen)? {
            break;
        }
    }
    disable_raw_mode().drop_error()?;
    execute!(screen, Show, LeaveAlternateScreen).drop_error()?;

    let (to_print, err_code) = if interrupted {
        ("".to_owned(), 1)
    } else {
        component.result()?
    };

    print!("{}", to_print);

    // std::process::exit is a divergent function
    std::process::exit(err_code as i32);
}
