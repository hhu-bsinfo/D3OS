#![no_std]
//! Stateful keyboard layout decoding for D3OS input consumers.

#[cfg(feature = "pinyin")]
extern crate alloc;

mod layout;
#[cfg(feature = "pinyin")]
pub mod pinyin;
#[cfg(feature = "userspace")]
mod userspace;
#[doc(hidden)]
pub mod wire;

#[cfg(feature = "userspace")]
pub use userspace::{get_layout, read_event, read_key, read_raw, set_layout};

pub use layout::Layout;
pub use pc_keyboard::{DecodedKey, KeyCode, KeyEvent, KeyState};
use pc_keyboard::{EventDecoder, HandleControl, layouts::AnyLayout};

/// Decodes one ordered stream of keyboard events. Create one decoder per stream.
pub struct KeyboardDecoder {
    layout: Layout,
    decoder: EventDecoder<AnyLayout>,
    shortcut_keys: u8,
}

impl Default for KeyboardDecoder {
    fn default() -> Self {
        Self::new(Layout::default())
    }
}

impl KeyboardDecoder {
    /// Creates a decoder with the given layout and the standard modifier state.
    pub const fn new(layout: Layout) -> Self {
        Self {
            layout,
            decoder: EventDecoder::new(layout.mapping(), HandleControl::Ignore),
            shortcut_keys: 0,
        }
    }

    pub const fn layout(&self) -> Layout {
        self.layout
    }

    /// Switches layouts for subsequent events, preserving held modifiers and lock state.
    pub fn set_layout(&mut self, layout: Layout) {
        self.decoder.change_layout(layout.mapping());
        self.layout = layout;
    }

    /// Processes a raw event. Feed both press and release events in order.
    ///
    /// Text and control characters are returned as [`DecodedKey::Unicode`].
    /// Non-character keys such as arrows are returned as [`DecodedKey::RawKey`].
    /// An event such as a release can update state and return `None`.
    /// Pressing Alt and Shift together advances the layout once per chord.
    pub fn process_keyevent(&mut self, event: KeyEvent) -> Option<DecodedKey> {
        // Keep separate bits for the two sides so releasing one does not
        // clear a modifier that is still held on the other side.
        const LEFT_ALT: u8 = 1;
        const RIGHT_ALT: u8 = 2;
        const LEFT_SHIFT: u8 = 4;
        const RIGHT_SHIFT: u8 = 8;
        let chord = |keys| keys & (LEFT_ALT | RIGHT_ALT) != 0 && keys & (LEFT_SHIFT | RIGHT_SHIFT) != 0;
        let key = match event.code {
            KeyCode::LAlt => LEFT_ALT,
            KeyCode::RAltGr => RIGHT_ALT,
            KeyCode::LShift => LEFT_SHIFT,
            KeyCode::RShift => RIGHT_SHIFT,
            _ => 0,
        };

        let was_active = chord(self.shortcut_keys);
        match event.state {
            KeyState::Down => self.shortcut_keys |= key,
            KeyState::Up => self.shortcut_keys &= !key,
            KeyState::SingleShot => {}
        }
        if !was_active && chord(self.shortcut_keys) {
            self.set_layout(self.layout.next());
        }
        self.decoder.process_keyevent(event)
    }

    /// Produces both forms of the same event for consumers that select an input mode.
    pub fn decode_event(&mut self, raw: KeyEvent) -> KeyboardInput {
        let decoded = self.process_keyevent(raw.clone());
        KeyboardInput { raw, decoded }
    }
}

/// A raw keyboard event and its mapping under the active layout.
pub struct KeyboardInput {
    pub raw: KeyEvent,
    pub decoded: Option<DecodedKey>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn switches_layout_with_shift_held() {
        let mut decoder = KeyboardDecoder::new(Layout::English);
        let y = KeyEvent::new(KeyCode::Y, KeyState::Down);
        assert_eq!(decoder.process_keyevent(y.clone()), Some(DecodedKey::Unicode('y')));

        decoder.process_keyevent(KeyEvent::new(KeyCode::LShift, KeyState::Down));
        decoder.set_layout(Layout::Deutsch);
        assert_eq!(decoder.layout(), Layout::Deutsch);
        assert_eq!(decoder.process_keyevent(y.clone()), Some(DecodedKey::Unicode('Z')));
        assert_eq!(decoder.process_keyevent(KeyEvent::new(KeyCode::LShift, KeyState::Up)), None);
        assert_eq!(decoder.process_keyevent(y.clone()), Some(DecodedKey::Unicode('z')));

        decoder.set_layout(Layout::ChinesePinyin);
        assert_eq!(decoder.process_keyevent(y), Some(DecodedKey::Unicode('y')));
    }

    #[test]
    fn alt_shift_cycles_once_until_the_chord_is_released() {
        let mut decoder = KeyboardDecoder::default();
        assert_eq!(decoder.layout(), Layout::Deutsch);

        decoder.process_keyevent(KeyEvent::new(KeyCode::LAlt, KeyState::Down));
        assert_eq!(decoder.layout(), Layout::Deutsch);

        for expected in [Layout::English, Layout::ChinesePinyin, Layout::Deutsch, Layout::English] {
            decoder.process_keyevent(KeyEvent::new(KeyCode::RShift, KeyState::Down));
            assert_eq!(decoder.layout(), expected);

            // Typematic repeats and pressing/releasing the other Shift while
            // the first is held leave the chord active.
            decoder.process_keyevent(KeyEvent::new(KeyCode::RShift, KeyState::Down));
            decoder.process_keyevent(KeyEvent::new(KeyCode::LShift, KeyState::Down));
            decoder.process_keyevent(KeyEvent::new(KeyCode::LShift, KeyState::Up));
            assert_eq!(decoder.layout(), expected);

            decoder.process_keyevent(KeyEvent::new(KeyCode::RShift, KeyState::Up));
            assert_eq!(decoder.layout(), expected);
        }
    }

    #[test]
    fn right_alt_can_complete_the_chord_after_shift() {
        let mut decoder = KeyboardDecoder::default();
        decoder.process_keyevent(KeyEvent::new(KeyCode::LShift, KeyState::Down));
        decoder.process_keyevent(KeyEvent::new(KeyCode::RAltGr, KeyState::Down));
        assert_eq!(decoder.layout(), Layout::English);
    }

    #[test]
    fn wire_preserves_raw_events_and_unicode() {
        let raw = KeyEvent::new(KeyCode::E, KeyState::Down);
        for decoded in [
            None,
            Some(DecodedKey::Unicode('€')),
            Some(DecodedKey::Unicode('中')),
            Some(DecodedKey::RawKey(KeyCode::ArrowLeft)),
        ] {
            let value = wire::encode_event(KeyboardInput { raw: raw.clone(), decoded });
            let input = wire::decode_event(value).unwrap();
            assert_eq!(input.raw, raw);
            assert_eq!(input.decoded, decoded);
        }
        assert!(wire::decode_event(0).is_none());
    }

    #[test]
    fn preserves_lock_state_and_decodes_altgr_and_non_text_keys() {
        let mut decoder = KeyboardDecoder::new(Layout::English);
        decoder.process_keyevent(KeyEvent::new(KeyCode::CapsLock, KeyState::Down));
        decoder.process_keyevent(KeyEvent::new(KeyCode::CapsLock, KeyState::Up));
        decoder.set_layout(Layout::Deutsch);
        assert_eq!(
            decoder.process_keyevent(KeyEvent::new(KeyCode::Oem4, KeyState::Down)),
            Some(DecodedKey::Unicode('Ü'))
        );
        decoder.process_keyevent(KeyEvent::new(KeyCode::RAltGr, KeyState::Down));
        assert_eq!(
            decoder.process_keyevent(KeyEvent::new(KeyCode::Q, KeyState::Down)),
            Some(DecodedKey::Unicode('@'))
        );
        assert_eq!(
            decoder.process_keyevent(KeyEvent::new(KeyCode::ArrowLeft, KeyState::Down)),
            Some(DecodedKey::RawKey(KeyCode::ArrowLeft))
        );
    }
}
