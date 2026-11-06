use core::cell::RefCell;

use alloc::{rc::Rc, vec::Vec};
use globals::hotkeys::{
    HKEY_CLOSE_TAB, HKEY_NEW_TAB, HKEY_NEXT_TAB, HKEY_PREV_TAB, HKEY_TOGGLE_TERMINAL_WINDOW,
};
use pc_keyboard::layouts::{AnyLayout, De105Key};
use pc_keyboard::{DecodedKey, EventDecoder, HandleControl, KeyCode, KeyEvent};
use stream::{event_to_u16, RawInputStream};
use terminal_lib::manager::cmd;
use terminal_lib::session::{CtlRecord, WaitState};
use terminal_lib::{DecodedKeyType, TerminalMode};

use crate::{
    event_handler::{Event, EventHandler},
    sessions::SessionMux,
    terminal::lfb_terminal::LFBTerminal,
    worker::canonical::CanonicalAction,
};

use super::worker::Worker;

pub struct InputObserver {
    terminal: Rc<LFBTerminal>,
    event_handler: Rc<RefCell<EventHandler>>,
    decoder: EventDecoder<AnyLayout>,
    mode: TerminalMode,
    /// Shared session multiplexer: owns per-session endpoints, canonical
    /// editors, and routes input to the active session's foreground app.
    mux: Rc<RefCell<SessionMux>>,
}

impl InputObserver {
    pub fn new(
        terminal: Rc<LFBTerminal>,
        event_handler: Rc<RefCell<EventHandler>>,
        mux: Rc<RefCell<SessionMux>>,
    ) -> Self {
        Self {
            terminal,
            event_handler,
            decoder: EventDecoder::new(AnyLayout::De105Key(De105Key), HandleControl::Ignore),
            mode: TerminalMode::Raw,
            mux,
        }
    }

    /// Resolve the current foreground input mode from the control record.
    /// `None` means no foreground app is currently waiting for input.
    fn resolve_mode(&self, record: Option<CtlRecord>) -> Option<TerminalMode> {
        match record {
            Some(record) if record.wait == WaitState::Waiting => Some(record.mode),
            _ => None,
        }
    }

    /// Deliver a decoded input buffer to the active session's foreground app.
    fn deliver(&mut self, buffer: &[u8]) {
        self.mux.borrow_mut().deliver(buffer);
    }
}

impl Worker for InputObserver {
    fn run(&mut self) {
        self.mux.borrow_mut().ensure_active_in_writer();

        let Some(key_event) = self.terminal.read_event_nb() else {
            return;
        };

        let record = self.mux.borrow().active_ctl();
        let use_pipe = record.is_some();
        let mode = self.resolve_mode(record);

        if let Some(mode) = mode {
            self.mode = mode;
        }

        // Process key event into decoded key (unicode char or raw keycode)
        let Some(decoded_key) = self.decoder.process_keyevent(key_event.clone()) else {
            // This returns none if the key event was a key release
            // In this case, we only process the byte if the terminal is in raw mode
            if use_pipe && self.mode == TerminalMode::Raw {
                if let Some(buffer) = self.buffer_raw(key_event) {
                    self.deliver(&buffer);
                }
            }

            return;
        };

        // Handle reserved keys (e.g. hotkeys)
        let Some(decoded_key) = self.try_intercept_reserved_key(decoded_key) else {
            return;
        };

        // Buffer the decoded key based on the terminal input mode. Canonical
        // mode edits and echoes through the active session's own state.
        match mode {
            Some(TerminalMode::Canonical) => {
                if let Some(action) = Self::canonical_action(decoded_key) {
                    self.mux.borrow_mut().handle_canonical(action);
                }
            }
            Some(TerminalMode::Fluid) => {
                if let Some(buffer) = self.buffer_fluid(decoded_key) {
                    self.deliver(&buffer);
                }
            }
            Some(TerminalMode::Raw) => {
                if let Some(buffer) = self.buffer_raw(key_event) {
                    self.deliver(&buffer);
                }
            }
            None => {}
        }
    }
}

impl InputObserver {
    fn try_intercept_reserved_key(&self, key: DecodedKey) -> Option<DecodedKey> {
        match key {
            DecodedKey::RawKey(HKEY_TOGGLE_TERMINAL_WINDOW) => {
                self.event_handler.borrow_mut().trigger(Event::EnterGuiMode);
                None
            }
            DecodedKey::RawKey(HKEY_NEW_TAB) => {
                self.mux.borrow_mut().send_command(cmd::NEW_TAB);
                None
            }
            DecodedKey::RawKey(HKEY_CLOSE_TAB) => {
                self.mux.borrow_mut().send_command(cmd::CLOSE_ACTIVE);
                None
            }
            DecodedKey::RawKey(HKEY_NEXT_TAB) => {
                self.mux.borrow_mut().send_command(cmd::NEXT_TAB);
                None
            }
            DecodedKey::RawKey(HKEY_PREV_TAB) => {
                self.mux.borrow_mut().send_command(cmd::PREV_TAB);
                None
            }
            key => Some(key),
        }
    }

    fn buffer_raw(&self, event: KeyEvent) -> Option<Vec<u8>> {
        let raw = event_to_u16(event);
        Some(raw.to_ne_bytes().to_vec())
    }

    fn buffer_fluid(&self, key: DecodedKey) -> Option<Vec<u8>> {
        match key {
            DecodedKey::Unicode(key) => Some([DecodedKeyType::Unicode as u8, key as u8].to_vec()),
            DecodedKey::RawKey(key) => Some([DecodedKeyType::RawKey as u8, key as u8].to_vec()),
        }
    }

    /// Map a decoded key to a canonical-mode edit action, or `None` when the key
    /// has no canonical effect.
    fn canonical_action(key: DecodedKey) -> Option<CanonicalAction> {
        match key {
            DecodedKey::RawKey(KeyCode::ArrowLeft) => Some(CanonicalAction::Left),
            DecodedKey::RawKey(KeyCode::ArrowRight) => Some(CanonicalAction::Right),
            DecodedKey::RawKey(KeyCode::Home) => Some(CanonicalAction::Home),
            DecodedKey::RawKey(KeyCode::End) => Some(CanonicalAction::End),
            DecodedKey::RawKey(_) => None,
            DecodedKey::Unicode('\x1B') => None,
            DecodedKey::Unicode('\n') => Some(CanonicalAction::Submit),
            DecodedKey::Unicode('\x08') => Some(CanonicalAction::Backspace),
            DecodedKey::Unicode('\x7F') => Some(CanonicalAction::Delete),
            DecodedKey::Unicode(ch) => Some(CanonicalAction::Insert(ch)),
        }
    }
}
