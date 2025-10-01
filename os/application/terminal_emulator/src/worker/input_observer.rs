use core::cell::RefCell;

use alloc::{format, rc::Rc, string::String, vec::Vec};
use globals::hotkeys::HKEY_TOGGLE_TERMINAL_WINDOW;
use pc_keyboard::{DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use stream::{InputStream, OutputStream};
use syscall::{SystemCall, syscall};
use terminal_lib::{println, DecodedKeyType, TerminalInputState, TerminalMode};

use crate::{
    event_handler::{Event, EventHandler},
    terminal::lfb_terminal::LFBTerminal,
    util::decoder::Decoder,
};

use super::worker::Worker;

const BUFFER_SIZE: usize = 256;

struct Canonical {
    cursor_pos: usize,
    buffer: String,
}

impl Canonical {
    const fn new() -> Self {
        Self {
            cursor_pos: 0,
            buffer: String::new(),
        }
    }

    fn submit(&mut self) -> Vec<u8> {
        let buffer = self.buffer.clone();
        self.cursor_pos = 0;
        self.buffer.clear();
        buffer.into()
    }

    fn remove_at_cursor(&mut self) -> Result<(), ()> {
        if self.cursor_pos >= self.buffer.len() || self.buffer.is_empty() {
            return Err(());
        }
        self.buffer.remove(self.cursor_pos);
        Ok(())
    }

    fn remove_before_cursor(&mut self) -> Result<(), ()> {
        if self.cursor_pos <= 0 {
            return Err(());
        }
        self.buffer.remove(self.cursor_pos - 1);
        self.cursor_pos -= 1;
        Ok(())
    }

    fn add_at_cursor(&mut self, ch: char) -> Result<(), ()> {
        if self.buffer.len() >= BUFFER_SIZE {
            return Err(());
        }
        self.buffer.insert(self.cursor_pos, ch);
        self.cursor_pos += 1;
        Ok(())
    }

    fn move_cursor_to_start(&mut self) -> Result<usize, ()> {
        self.cursor_pos = 0;
        Ok(self.cursor_pos)
    }

    fn move_cursor_to_end(&mut self) -> Result<usize, ()> {
        self.cursor_pos = self.buffer.len();
        Ok(self.buffer.len() - self.cursor_pos)
    }

    fn move_cursor_left(&mut self) -> Result<(), ()> {
        if self.cursor_pos <= 0 {
            return Err(());
        }
        self.cursor_pos -= 1;
        Ok(())
    }

    fn move_cursor_right(&mut self) -> Result<(), ()> {
        if self.cursor_pos >= self.buffer.len() {
            return Err(());
        }
        self.cursor_pos += 1;
        Ok(())
    }
}

pub struct InputObserver {
    terminal: Rc<LFBTerminal>,
    event_handler: Rc<RefCell<EventHandler>>,
    decoder: Keyboard<AnyLayout, ScancodeSet1>,
    mode: TerminalMode,
    canonical: Canonical,
}

impl InputObserver {
    pub const fn new(terminal: Rc<LFBTerminal>, event_handler: Rc<RefCell<EventHandler>>) -> Self {
        Self {
            terminal,
            event_handler,
            decoder: Keyboard::new(
                ScancodeSet1::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            ),
            mode: TerminalMode::Raw,
            canonical: Canonical::new(),
        }
    }
}

impl Worker for InputObserver {
    fn run(&mut self) {
        let raw = self.terminal.read_byte_nb().unwrap_or_default() as u8;

        // Process raw keyboard byte into key event (keycode + state up/down)
        let Ok(Some(key_event)) = self.decoder.add_byte(raw) else {
            return;
        };

        // Get terminal input state (canonical, fluid, idle)
        let raw_state = syscall(SystemCall::TerminalCheckInputState, &[]).expect("Unable to check input state");
        let state = TerminalInputState::from(raw_state);

        // Process key event into decoded key (unicode char or raw keycode)
        let Some(decoded_key) = self.decoder.process_keyevent(key_event) else {
            // This returns none if the key event was a key release
            // In this case, we only process the byte if the terminal is in raw mode
            if self.mode == TerminalMode::Raw {
                if let Some(buffer) = self.buffer_raw(raw) {
                    syscall(
                        SystemCall::TerminalWriteInput,
                        &[buffer.as_ptr() as usize, buffer.len(), TerminalMode::Raw as usize],
                    );
                }
            }

            return;
        };

        // Handle reserved keys (e.g. hotkeys)
        let Some(decoded_key) = self.try_intercept_reserved_key(decoded_key) else {
            return;
        };

        // Buffer the decoded key based on the terminal input state
        let (buffer, mode) = match state {
            TerminalInputState::Canonical => (self.buffer_canonical(decoded_key), TerminalMode::Canonical),
            TerminalInputState::Fluid => (self.buffer_fluid(decoded_key), TerminalMode::Fluid),
            TerminalInputState::Raw => (self.buffer_raw(raw), TerminalMode::Raw),
            TerminalInputState::Idle => return
        };
        let Some(buffer) = buffer else {
            return;
        };

        syscall(
            SystemCall::TerminalWriteInput,
            &[buffer.as_ptr() as usize, buffer.len(), mode as usize],
        );
    }
}

impl InputObserver {
    fn try_intercept_reserved_key(&self, key: DecodedKey) -> Option<DecodedKey> {
        match key {
            DecodedKey::RawKey(HKEY_TOGGLE_TERMINAL_WINDOW) => {
                self.event_handler.borrow_mut().trigger(Event::EnterGuiMode);
                return None;
            }
            key => return Some(key),
        }
    }

    fn buffer_raw(&self, raw: u8) -> Option<Vec<u8>> {
        Some([raw].to_vec())
    }

    fn buffer_fluid(&self, key: DecodedKey) -> Option<Vec<u8>> {
        match key {
            DecodedKey::Unicode(key) => Some([DecodedKeyType::Unicode as u8, key as u8].to_vec()),
            DecodedKey::RawKey(key) => Some([DecodedKeyType::RawKey as u8, key as u8].to_vec()),
        }
    }

    fn buffer_canonical(&mut self, key: DecodedKey) -> Option<Vec<u8>> {
        match key {
            DecodedKey::RawKey(KeyCode::ArrowLeft) => {
                if self.canonical.move_cursor_left().is_ok() {
                    self.terminal.write_str("\x1b[1D");
                }
            }
            DecodedKey::RawKey(KeyCode::ArrowRight) => {
                if self.canonical.move_cursor_right().is_ok() {
                    self.terminal.write_str("\x1b[1C");
                }
            }
            DecodedKey::RawKey(KeyCode::Home) => {
                if let Ok(steps) = self.canonical.move_cursor_to_start() {
                    self.terminal.write_str(&format!("\x1b[{}D", steps));
                }
            }
            DecodedKey::RawKey(KeyCode::End) => {
                if let Ok(steps) = self.canonical.move_cursor_to_end() {
                    self.terminal.write_str(&format!("\x1b[{}C", steps));
                }
            }
            DecodedKey::RawKey(_) => return None,

            DecodedKey::Unicode('\x1B') => return None,
            DecodedKey::Unicode('\n') => {
                let offset = self.canonical.buffer.len() - self.canonical.cursor_pos;
                if offset > 0 {
                    self.terminal.write_str(&format!("\x1B[{}C\n", offset));
                } else {
                    self.terminal.write_byte(b'\n');
                }
                return Some(self.canonical.submit());
            }
            DecodedKey::Unicode('\x08') => {
                if self.canonical.remove_before_cursor().is_ok() {
                    self.terminal
                        .write_str(&format!("\x1B[1D \x1B[1D{}", self.redraw_canonical_content()));
                }
            }
            DecodedKey::Unicode('\x7F') => {
                if self.canonical.remove_at_cursor().is_ok() {
                    self.terminal
                        .write_str(&format!(" \x1B[1D{}", self.redraw_canonical_content()));
                }
            }
            DecodedKey::Unicode(ch) => {
                if self.canonical.add_at_cursor(ch).is_ok() {
                    self.terminal
                        .write_str(&format!("{}{}", ch, self.redraw_canonical_content()));
                }
            }
        };
        None
    }

    fn redraw_canonical_content(&self) -> String {
        let content = &self.canonical.buffer[self.canonical.cursor_pos..];
        if content.is_empty() {
            String::new()
        } else {
            format!("\x1b[0K{}\x1B[{}D", content, content.len())
        }
    }
}
