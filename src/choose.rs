use std::num::NonZeroUsize;

use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode, KeyEvent},
    execute,
    style::{Attribute, Print, ResetColor, SetAttribute, SetForegroundColor},
};
use lru::LruCache;

use crate::{component::ComponentTrait, get_bg_color, DropError};

#[derive(Debug)]
pub(crate) struct Choose {
    pub text: String,
    pub selected_string: String,
    pub unselected_string: String,
    pub inexact: bool,
    pub choices: Vec<String>,
    pub chosen: LruCache<usize, ()>,
    pub selections: NonZeroUsize,
    pub cursor_loc: usize,
}

impl ComponentTrait for Choose {
    fn result(self) -> Result<String, u8> {
        let s = self
            .chosen
            .iter()
            .filter_map(|(k, _)| self.choices.get(*k).map(ToOwned::to_owned))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(s)
    }

    fn handle_event(
        &mut self,
        event: &crossterm::event::Event,
        _screen: &mut std::io::Stderr,
    ) -> Result<bool, ()> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Down,
                ..
            }) => {
                if self.cursor_loc != self.choices.len() - 1 {
                    self.cursor_loc += 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Up, ..
            }) => {
                if self.cursor_loc != 0 {
                    self.cursor_loc -= 1;
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Char(' '),
                ..
            }) => {
                let curself = self.chosen.get(&self.cursor_loc).is_some();
                if curself {
                    // Remove from selection
                    self.chosen.pop(&self.cursor_loc);
                } else {
                    // Add to selection
                    self.chosen.push(self.cursor_loc, ());
                }
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                ..
            }) => {
                if self.inexact || self.chosen.len() == self.selections.get() {
                    return Ok(true);
                }
            }
            _ => {}
        };

        Ok(false)
    }

    fn draw(&mut self, screen: &mut std::io::Stderr) -> Result<(), ()> {
        let padding = 2;
        let mut line = padding;
        execute!(
            screen,
            MoveTo(padding, line),
            Print(&self.text),
            MoveTo(padding, line + 1),
            SetAttribute(Attribute::Dim),
            SetAttribute(Attribute::Italic),
            Print(format!(
                "Select {} {}",
                if self.inexact { "up to" } else { "exactly" },
                self.selections.get()
            )),
            SetAttribute(Attribute::Reset)
        )
        .drop_error()?;

        line += 3;
        for (choice_i, choice) in self.choices.iter().enumerate() {
            if choice_i == self.cursor_loc {
                execute!(screen, SetForegroundColor(get_bg_color(true))).drop_error()?;
            }

            let selection: &str = if self.chosen.contains(&choice_i) {
                &self.selected_string
            } else {
                &self.unselected_string
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
