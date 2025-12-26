use anyhow::Result;
use arboard::Clipboard;
use enigo::{Enigo, Key, Keyboard, Settings};
use log::{debug, error};
use std::thread;
use std::time::Duration;

pub struct TextInserter {
    clipboard: Clipboard,
    enigo: Enigo,
}

impl TextInserter {
    pub fn new() -> Result<Self> {
        let clipboard = Clipboard::new()?;
        let enigo = Enigo::new(&Settings::default())?;

        Ok(Self {
            clipboard,
            enigo,
        })
    }

    pub fn insert_text(&mut self, text: &str) -> Result<()> {
        debug!("Inserting text: {} chars", text.len());

        let old_clipboard = self.clipboard.get_text().ok();

        self.clipboard.set_text(text)?;

        thread::sleep(Duration::from_millis(50));

        self.enigo.key(Key::Control, enigo::Direction::Press)?;
        self.enigo.key(Key::Unicode('v'), enigo::Direction::Click)?;
        self.enigo.key(Key::Control, enigo::Direction::Release)?;

        thread::sleep(Duration::from_millis(100));

        if let Some(old_text) = old_clipboard {
            if let Err(e) = self.clipboard.set_text(&old_text) {
                error!("Failed to restore clipboard: {}", e);
            }
        }

        Ok(())
    }

    pub fn copy_to_clipboard(&mut self, text: &str) -> Result<()> {
        debug!("Copying to clipboard: {} chars", text.len());
        self.clipboard.set_text(text)?;
        Ok(())
    }
}
