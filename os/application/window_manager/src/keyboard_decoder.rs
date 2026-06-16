use input::keyboard;
use pc_keyboard::{layouts::{AnyLayout, De105Key}, EventDecoder, HandleControl};
use spin::mutex::Mutex;
use terminal::DecodedKey;

/// Author: Sebastian Keller
///
/// Keyboard decoder that reads directly from the keyboard.
/// Reading from the terminal caused a heavy performance hit, due to the current lack of ipc tools.
pub struct KeyboardDecoder {
    decoder: Mutex<EventDecoder<AnyLayout>>,
}

impl KeyboardDecoder {
    pub const fn new() -> Self {
        Self {
            decoder: Mutex::new(EventDecoder::new(
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            )),
        }
    }

    /// Author: Sebastian Keller
    ///
    /// Returns: Decoded key of user input (Unicode or Keycode)
    ///
    /// Returns: None if there is no user input
    pub fn read_decoded(&self) -> Option<DecodedKey> {
        let mut decoder = self.decoder.lock();

        keyboard::read_raw(false).map(|ev| decoder.process_keyevent(ev)).flatten()
    }
}
