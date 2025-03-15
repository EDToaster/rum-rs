mod choose;
mod component;
mod confirm;
mod spinner;
mod text;
mod typer;

use std::{io::stderr, num::NonZeroUsize, process::ExitCode, time::Duration};

use clap::{Parser, Subcommand};
use component::{Component, ComponentTrait};
use crossterm::{
    cursor::{Hide, Show},
    event::{poll, read, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::Color,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

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
    subcommand: CommandOpt,
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
enum CommandOpt {
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
