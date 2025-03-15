use std::{
    process::Child,
    time::{Duration, Instant},
};

use crossterm::{cursor::MoveTo, execute, style::Print};

use crate::{ComponentTrait, DropError};

#[derive(Debug)]
pub(crate) struct Spinner {
    pub speed: Duration,
    pub text: String,
    pub child: Child,
    pub chars: Vec<String>,
    pub progress: usize,
    pub last_updated: Instant,
}

impl ComponentTrait for Spinner {
    fn result(self) -> Result<String, u8> {
        let mut child = self.child;
        // Assume that child is already finished
        // TODO: better ergo
        let output = child.try_wait().map_err(|_e| 1)?;
        if let Some(code) = output {
            if code.success() {
                Ok(String::new())
            } else {
                Err(code.code().unwrap_or(1) as u8)
            }
        } else {
            child.kill().ok(); // swallow error
            Err(1)
        }
    }

    fn tick(&mut self, _screen: &mut std::io::Stderr) -> Result<bool, ()> {
        if let Some(_) = self.child.try_wait().drop_error()? {
            return Ok(true);
        }

        if self.last_updated.elapsed() > self.speed {
            // Update progress
            self.progress = (self.progress + 1) % self.chars.len();
            self.last_updated = Instant::now();
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

    fn draw(&mut self, screen: &mut std::io::Stderr) -> Result<(), ()> {
        let padding = 2;
        let c = &self.chars[self.progress];
        execute!(
            screen,
            MoveTo(padding, padding),
            Print(format!("{c}  {}", self.text)),
        )
        .drop_error()
    }
}
