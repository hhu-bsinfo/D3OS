//! Client for the system Pinyin daemon. The foreground input owner forwards events.

mod mailbox;

use alloc::string::String;
pub use mailbox::Mailbox;

use crate::{DecodedKey, KeyCode, KeyState, KeyboardInput, Layout};

pub const TOGGLE: u8 = 1;
pub const RESET: u8 = 2;
pub const PASS: u8 = 1;
pub const CHINESE: u8 = 2;

#[derive(Default)]
pub struct PinyinClient {
    mailbox: Option<Mailbox>,
    layout: Option<Layout>,
    reset: bool,
    shift: ShiftTap,
    chinese: Option<bool>,
    status: String,
}

pub enum Delivery {
    Pass,
    /// The IME consumed the event. A nonempty string is committed to the target.
    Commit(String),
}

impl PinyinClient {
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Discard preedit when the input target changes.
    pub fn cancel(&mut self) {
        self.reset = true;
        self.status.truncate(self.status.len().min(6));
    }

    pub fn suspend(&mut self) {
        self.cancel();
        self.shift = ShiftTap::default();
        self.chinese = None;
        self.status.clear();
    }

    pub fn disconnect(&mut self) {
        self.mailbox = None;
    }

    pub fn sync_layout(&mut self, layout: Option<Layout>) {
        if self.layout != layout {
            self.layout = layout;
            self.cancel();
            self.shift.pending = false;
            self.status.clear();
            self.chinese = None;
        }
    }

    pub fn handle(&mut self, input: &KeyboardInput, text_target: bool) -> Delivery {
        self.sync_layout(crate::get_layout().ok());
        let tap = self.shift.observe(input.raw.code, input.raw.state);
        if self.layout != Some(Layout::ChinesePinyin) {
            return Delivery::Pass;
        }

        if !tap && self.chinese == Some(false) {
            return Delivery::Pass;
        }

        let key = if text_target && self.shift.other == 0 {
            input
                .decoded
                .filter(|key| !matches!(key, DecodedKey::RawKey(KeyCode::LShift | KeyCode::RShift)))
        } else {
            if !text_target {
                self.cancel();
            }
            None
        };
        if !tap && key.is_none() && self.chinese.is_some() {
            return Delivery::Pass;
        }

        let flags = (if tap { TOGGLE } else { 0 }) | (if self.reset || self.chinese.is_none() { RESET } else { 0 });
        let Some((flags, committed, preview)) = self.request(flags, key) else {
            self.cancel();
            self.chinese = None;
            self.status.clear();
            return Delivery::Pass;
        };
        self.reset = false;
        let chinese = flags & CHINESE != 0;
        self.chinese = Some(chinese);
        self.status.clear();
        self.status.push_str(if chinese { "IME CN" } else { "IME EN" });
        if !preview.is_empty() {
            self.status.push(' ');
            self.status.push_str(&preview);
        }
        if flags & PASS != 0 { Delivery::Pass } else { Delivery::Commit(committed) }
    }

    fn request(&mut self, flags: u8, key: Option<DecodedKey>) -> Option<(u8, String, String)> {
        if self.mailbox.is_none() {
            self.mailbox = Mailbox::open();
        }
        self.mailbox.as_ref()?.exchange(flags, key)
    }
}

#[derive(Default)]
struct ShiftTap {
    held: u8,
    other: u8,
    pending: bool,
}

impl ShiftTap {
    fn observe(&mut self, code: KeyCode, state: KeyState) -> bool {
        let shift = match code {
            KeyCode::LShift => 1,
            KeyCode::RShift => 2,
            _ => 0,
        };
        let modifier = match code {
            KeyCode::LAlt => 1,
            KeyCode::RAltGr => 2,
            KeyCode::LControl => 4,
            KeyCode::RControl => 8,
            KeyCode::LWin => 16,
            KeyCode::RWin => 32,
            _ => 0,
        };
        match state {
            KeyState::Down if shift != 0 => {
                if self.held & shift == 0 {
                    self.pending = self.held == 0 && self.other == 0;
                    self.held |= shift;
                }
            }
            KeyState::Up if shift != 0 => {
                let tap = self.pending && self.held == shift;
                self.held &= !shift;
                self.pending = false;
                return tap;
            }
            KeyState::Down => {
                self.other |= modifier;
                self.pending = false;
            }
            KeyState::Up => self.other &= !modifier,
            KeyState::SingleShot => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_tap_excludes_chords_and_repeats() {
        let mut shift = ShiftTap::default();
        assert!(!shift.observe(KeyCode::LShift, KeyState::Down));
        assert!(!shift.observe(KeyCode::LShift, KeyState::Down));
        assert!(shift.observe(KeyCode::LShift, KeyState::Up));
        shift.observe(KeyCode::RShift, KeyState::Down);
        shift.observe(KeyCode::A, KeyState::Down);
        assert!(!shift.observe(KeyCode::RShift, KeyState::Up));
        shift.observe(KeyCode::LAlt, KeyState::Down);
        shift.observe(KeyCode::LShift, KeyState::Down);
        shift.observe(KeyCode::LAlt, KeyState::Up);
        assert!(!shift.observe(KeyCode::LShift, KeyState::Up));
    }
}
