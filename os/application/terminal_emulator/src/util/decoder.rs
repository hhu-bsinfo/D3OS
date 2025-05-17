use pc_keyboard::{
    DecodedKey, HandleControl, Keyboard, ScancodeSet1,
    layouts::{AnyLayout, De105Key},
};

pub struct Decoder {
    decoder: Keyboard<AnyLayout, ScancodeSet1>,
}

impl Decoder {
    pub const fn new() -> Self {
        Self {
            decoder: Keyboard::new(
                ScancodeSet1::new(),
                AnyLayout::De105Key(De105Key),
                HandleControl::Ignore,
            ),
        }
    }

    pub fn decode(&mut self, raw: u8) -> Option<DecodedKey> {
        let event_option = match self.decoder.add_byte(raw) {
            Ok(event) => event,
            Err(_) => return None,
        };
        match event_option {
            Some(event) => self.decoder.process_keyevent(event),
            None => return None,
        }
    }
}
