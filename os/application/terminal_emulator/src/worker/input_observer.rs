use core::cell::RefCell;

use alloc::{rc::Rc, vec::Vec};
use globals::hotkeys::HKEY_TOGGLE_TERMINAL_WINDOW;
use pc_keyboard::DecodedKey;
use stream::{InputStream, OutputStream};
use syscall::{SystemCall, syscall};
use terminal_lib::{DecodedKeyType, TerminalInputState, TerminalMode};

use crate::{
    event_handler::{Event, EventHandler},
    terminal::lfb_terminal::LFBTerminal,
    util::decoder::Decoder,
};

use super::worker::Worker;

pub struct InputObserver {
    terminal: Rc<LFBTerminal>,
    event_handler: Rc<RefCell<EventHandler>>,
    decoder: Decoder,
    mode: TerminalMode,
    cooked_buffer: Vec<u8>,
}

impl InputObserver {
    pub const fn new(terminal: Rc<LFBTerminal>, event_handler: Rc<RefCell<EventHandler>>) -> Self {
        Self {
            terminal,
            event_handler,
            decoder: Decoder::new(),
            mode: TerminalMode::Raw,
            cooked_buffer: Vec::new(),
        }
    }
}

impl Worker for InputObserver {
    fn run(&mut self) {
        let raw = self.terminal.read_byte() as u8;
        let decoded = self.decoder.decode(raw);

        let decoded = match self.intercept(decoded) {
            Some(key) => key,
            None => {
                return;
            }
        };

        let state = TerminalInputState::from(syscall(SystemCall::TerminalCheckInputState, &[]).unwrap() as usize);

        let (buffer, mode) = match state {
            TerminalInputState::InputReaderAwaitsCooked => (self.buffer_cooked(decoded), TerminalMode::Cooked),
            TerminalInputState::InputReaderAwaitsMixed => (self.buffer_mixed(decoded), TerminalMode::Mixed),
            TerminalInputState::InputReaderAwaitsRaw => (self.buffer_raw(raw), TerminalMode::Raw),
            TerminalInputState::Idle => return,
        };

        let buffer = match buffer {
            Some(buffer) => buffer,
            None => return,
        };

        syscall(
            SystemCall::TerminalWriteInput,
            &[buffer.as_ptr() as usize, buffer.len(), mode as usize],
        );
    }
}

impl InputObserver {
    fn intercept(&self, key: Option<DecodedKey>) -> Option<DecodedKey> {
        if key.is_none() {
            return None;
        }

        match key.unwrap() {
            DecodedKey::RawKey(HKEY_TOGGLE_TERMINAL_WINDOW) => {
                self.event_handler.borrow_mut().trigger(Event::EnterGuiMode);
                return None;
            }
            key => return Some(key),
        }
    }

    fn buffer_raw(&mut self, raw: u8) -> Option<Vec<u8>> {
        Some([raw].to_vec())
    }

    fn buffer_mixed(&mut self, key: DecodedKey) -> Option<Vec<u8>> {
        match key {
            DecodedKey::Unicode(key) => Some([DecodedKeyType::Unicode as u8, key as u8].to_vec()),
            DecodedKey::RawKey(key) => Some([DecodedKeyType::RawKey as u8, key as u8].to_vec()),
        }
    }

    fn buffer_cooked(&mut self, key: DecodedKey) -> Option<Vec<u8>> {
        let ch = match key {
            DecodedKey::Unicode(ch) => ch,
            _ => return None,
        };

        self.terminal.write_byte(ch as u8);

        match ch {
            '\n' => {
                let buffer = self.cooked_buffer.clone();
                self.cooked_buffer.clear();
                return Some(buffer);
            }
            '\x08' => {
                if self.cooked_buffer.pop().is_some() {
                    self.terminal.write_str("\x1b[1D \x1b[1D");
                }
            }
            _ => self.cooked_buffer.push(ch as u8),
        };
        None
    }
}
