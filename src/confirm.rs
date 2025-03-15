use crossterm::{
    cursor::MoveTo,
    event::{Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Print, ResetColor, SetBackgroundColor},
};

use crate::{component::ComponentTrait, get_bg_color, DropError as _};

#[derive(Debug)]
pub(crate) struct Confirm {
    pub confirmed: bool,
    pub text: String,
    pub padded_no: String,
    pub padded_yes: String,
}

impl ComponentTrait for Confirm {
    fn result(self) -> Result<String, u8> {
        if self.confirmed {
            Ok(String::new())
        } else {
            Err(1)
        }
    }

    fn handle_event(
        &mut self,
        event: &crossterm::event::Event,
        _screen: &mut std::io::Stderr,
    ) -> Result<bool, ()> {
        match event {
            Event::Key(KeyEvent {
                code: KeyCode::Right,
                ..
            }) => self.confirmed = true,

            Event::Key(KeyEvent {
                code: KeyCode::Left,
                ..
            }) => self.confirmed = false,

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
        execute!(
            screen,
            MoveTo(padding, padding),
            Print(&self.text),
            MoveTo(padding, padding + 2),
            SetBackgroundColor(get_bg_color(!self.confirmed)),
            Print(&self.padded_no),
            ResetColor,
            Print("  "),
            SetBackgroundColor(get_bg_color(self.confirmed)),
            Print(&self.padded_yes),
            ResetColor
        )
        .drop_error()
    }
}
