use core::cell::RefCell;

use alloc::{rc::Rc, sync::Arc, vec::Vec};
use concurrent::thread::{self, Thread};
use globals::hotkeys::HKEY_TOGGLE_TERMINAL_WINDOW;
use pc_keyboard::DecodedKey;
use spin::Mutex;
use stream::{InputStream, OutputStream};
use syscall::{SystemCall, syscall};
use terminal_lib::{DecodedKeyType, TerminalInputState, TerminalMode};

use crate::{
    TerminalEmulator,
    event_handler::{Event, EventHandler},
    terminal::lfb_terminal::LFBTerminal,
    util::decoder::Decoder,
};

use super::worker::Worker;

pub struct InputObserver {
    terminal: Rc<RefCell<LFBTerminal>>,
    event_handler: Rc<RefCell<EventHandler>>,
    decoder: Decoder,
    mode: TerminalMode,
}

impl InputObserver {
    pub const fn new(
        terminal: Rc<RefCell<LFBTerminal>>,
        event_handler: Rc<RefCell<EventHandler>>,
    ) -> Self {
        Self {
            terminal,
            event_handler,
            decoder: Decoder::new(),
            mode: TerminalMode::Raw,
        }
    }
}

impl Worker for InputObserver {
    fn run(&mut self) {
        let terminal = self.terminal.borrow();
        let raw = terminal.read_byte() as u8;
        let decoded = self.decoder.decode(raw);

        let decoded = match self.intercept(decoded) {
            Some(key) => key,
            None => {
                return;
            }
        };

        let state = TerminalInputState::from(
            syscall(SystemCall::TerminalCheckInputState, &[]).unwrap() as usize,
        );

        let (buffer, mode) = match state {
            TerminalInputState::InputReaderAwaitsCooked => {
                (self.buffer_cooked(decoded), TerminalMode::Cooked)
            }
            TerminalInputState::InputReaderAwaitsMixed => {
                (self.buffer_mixed(decoded), TerminalMode::Mixed)
            }
            TerminalInputState::InputReaderAwaitsRaw => (self.buffer_raw(raw), TerminalMode::Raw),
            TerminalInputState::Idle => return,
        };

        syscall(
            SystemCall::TerminalWriteInput,
            &[buffer.as_ptr() as usize, buffer.len(), mode as usize],
        );
    }
}

impl InputObserver {
    // TODO Does not work once we entered cooked mode loop (intercept also there or only read from intercepted function)
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

    fn buffer_raw(&self, raw: u8) -> Vec<u8> {
        [raw].to_vec()
    }

    fn buffer_mixed(&self, key: DecodedKey) -> Vec<u8> {
        match key {
            DecodedKey::Unicode(key) => [DecodedKeyType::Unicode as u8, key as u8].to_vec(),
            DecodedKey::RawKey(key) => [DecodedKeyType::RawKey as u8, key as u8].to_vec(),
        }
    }

    fn buffer_cooked(&self, first_key: DecodedKey) -> Vec<u8> {
        let terminal = self.terminal.borrow();
        let mut buffer: Vec<u8> = Vec::new();
        // TODO#1 FIX
        // match first_key {
        //     DecodedKey::Unicode('\x08') => {}
        //     DecodedKey::Unicode('\n') => {
        //         terminal.write_byte('\n' as u8);
        //         return buffer;
        //     }
        //     DecodedKey::Unicode(ch) => {
        //         terminal.write_byte(ch as u8);
        //         buffer.push(ch as u8);
        //     }
        //     _ => {}
        // }

        // loop {
        //     let raw = terminal.read_byte() as u8;
        //     let ch = match self.decoder.decode(raw) {
        //         Some(DecodedKey::Unicode(ch)) => ch,
        //         _ => continue,
        //     };

        //     terminal.write_byte(ch as u8);

        //     match ch {
        //         '\n' => break,
        //         '\x08' => {
        //             if buffer.pop().is_some() {
        //                 terminal.write_str("\x1b[1D \x1b[1D");
        //             }
        //         }
        //         _ => buffer.push(ch as u8),
        //     };
        // }
        buffer
    }
}
