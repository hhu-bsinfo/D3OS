use input::keyboard;
use pc_keyboard::{
    layouts::{AnyLayout, De105Key},
    HandleControl, Keyboard, ScancodeSet1,
};
use spin::mutex::Mutex;
use terminal::DecodedKey;

/// Author: Sebastian Keller
///
/// Keyboard decoder that reads directly from the keyboard.
/// Reading from the terminal caused a heavy performance hit, due to the current lack of ipc tools.
pub struct KeyboardDecoder {
    decoder: Mutex<Keyboard<AnyLayout, ScancodeSet1>>,
}

impl KeyboardDecoder {
    pub const fn new() -> Self {
        Self {
            decoder: Mutex::new(Keyboard::new(
                ScancodeSet1::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            )),
        }
    }

    fn read_byte(&self) -> i16 {
        match keyboard::read_raw(false) {
            Some(byte) => byte as i16,
            None => 0,
        }
    }

    /// Author: Sebastian Keller
    ///
    /// Returns: Byte of raw keycode user input
    ///
    /// Returns: None if there is no user input
    pub fn read_raw(&self) -> Option<u8> {
        match self.read_byte() {
            ..0 => return None,
            byte => Some(byte as u8),
        }
    }

    /// Author: Sebastian Keller
    ///
    /// Returns: Decoded key of user input (Unicode or Keycode)
    ///
    /// Returns: None if there is no user input
    pub fn read_decoded(&self) -> Option<DecodedKey> {
        let mut decoder = self.decoder.lock();

        let byte = match self.read_raw() {
            Some(byte) => byte,
            None => return None,
        };
        let event_option = match decoder.add_byte(byte) {
            Ok(event) => event,
            Err(_) => return None,
        };
        match event_option {
            Some(event) => decoder.process_keyevent(event),
            None => return None,
        }
    }
}
