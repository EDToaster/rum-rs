use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Attribute, Print, SetAttribute},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{ComponentTrait, DropError as _};

#[derive(Debug)]
pub(crate) struct Text {
    pub width: usize,
    pub placeholder: String,
    pub prefix: String,
    pub input: String,
}

impl ComponentTrait for Text {
    fn result(self) -> Result<String, u8> {
        Ok(self.input)
    }

    fn handle_event(
        &mut self,
        event: &crossterm::event::Event,
        _screen: &mut std::io::Stderr,
    ) -> Result<bool, ()> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Char(c),
                ..
            }) => self.input.push(*c),
            Event::Key(KeyEvent {
                code: KeyCode::Backspace,
                ..
            }) => {
                self.input.pop();
            }
            Event::Key(KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
                ..
            }) => return Ok(true),
            _ => {}
        };

        Ok(false)
    }

    fn draw(&mut self, screen: &mut std::io::Stderr) -> Result<(), ()> {
        let padding = 2;
        execute!(screen, MoveTo(padding, padding)).drop_error()?;

        let (is_bg, to_print) = match self.input.as_str() {
            "" => {
                // show first n graphemes of placeholder
                let end = self
                    .placeholder
                    .grapheme_indices(true)
                    .nth(self.width)
                    .map(|(i, _)| i)
                    .unwrap_or(self.placeholder.len());
                (true, &self.placeholder[..end])
            }
            s => {
                // show last n graphemes of input
                let start = s
                    .grapheme_indices(true)
                    .rev()
                    .nth(self.width - 1)
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
            Print(&self.prefix),
            Print(to_print),
            SetAttribute(Attribute::Reset)
        )
        .drop_error()
    }
}
