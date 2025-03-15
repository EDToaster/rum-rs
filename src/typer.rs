use std::time::{Duration, Instant};

use crossterm::{execute, style::Print};

use crate::{component::ComponentTrait, DropError};

#[derive(Debug)]
pub(crate) struct Typer {
    pub speed: Duration,
    pub wait: Duration,
    /// reverse order of the string to type
    pub graphemes: Vec<String>,
    pub done_printing: bool,
    pub last_updated: Instant,
}

impl ComponentTrait for Typer {
    fn result(self) -> Result<String, u8> {
        Ok(String::new())
    }

    fn tick(&mut self, screen: &mut std::io::Stderr) -> Result<bool, ()> {
        if self.done_printing {
            if self.last_updated.elapsed() > self.wait {
                return Ok(true);
            }
        } else {
            if self.last_updated.elapsed() > self.speed {
                if let Some(c) = self.graphemes.pop() {
                    execute!(screen, Print(c)).drop_error()?;
                    self.last_updated = Instant::now();
                } else {
                    self.done_printing = true;
                }
            }
        }

        Ok(false)
    }

    fn handle_event(
        &mut self,
        _event: &crossterm::event::Event,
        _screen: &mut std::io::Stderr,
    ) -> Result<bool, ()> {
        Ok(false)
    }

    fn draw(&mut self, _screen: &mut std::io::Stderr) -> Result<(), ()> {
        Ok(())
    }
}
