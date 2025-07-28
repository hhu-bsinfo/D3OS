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
    canonical_buffer: Vec<u8>,
}

impl InputObserver {
    pub const fn new(terminal: Rc<LFBTerminal>, event_handler: Rc<RefCell<EventHandler>>) -> Self {
        Self {
            terminal,
            event_handler,
            decoder: Decoder::new(),
            mode: TerminalMode::Raw,
            canonical_buffer: Vec::new(),
        }
    }
}

impl Worker for InputObserver {
    fn run(&mut self) {
        let raw = self.terminal.read_byte() as u8;
        let Some(decoded_key) = self.decoder.decode(raw) else {
            return;
        };
        let Some(decoded_key) = self.try_intercept_reserved_key(decoded_key) else {
            return;
        };

        let raw_state = syscall(SystemCall::TerminalCheckInputState, &[]).expect("Unable to check input state");
        let state = TerminalInputState::from(raw_state);

        let (buffer, mode) = match state {
            TerminalInputState::Canonical => (self.buffer_canonical(decoded_key), TerminalMode::Canonical),
            TerminalInputState::Fluid => (self.buffer_fluid(decoded_key), TerminalMode::Fluid),
            TerminalInputState::Raw => (self.buffer_raw(raw), TerminalMode::Raw),
            TerminalInputState::Idle => return,
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

    // TODO add command line editing
    fn buffer_canonical(&mut self, key: DecodedKey) -> Option<Vec<u8>> {
        let ch = match key {
            DecodedKey::Unicode(ch) => ch,
            _ => return None,
        };

        self.terminal.write_byte(ch as u8);

        match ch {
            '\n' => {
                let buffer = self.canonical_buffer.clone();
                self.canonical_buffer.clear();
                return Some(buffer);
            }
            '\x08' => {
                if self.canonical_buffer.pop().is_some() {
                    self.terminal.write_str("\x1b[1D \x1b[1D");
                }
            }
            _ => self.canonical_buffer.push(ch as u8),
        };
        None
    }
}
