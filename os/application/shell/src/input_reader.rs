use concurrent::thread;
use terminal::{DecodedKey, print, read::read_mixed};

pub struct InputReader {}

impl InputReader {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn read(&self) -> DecodedKey {
        let key = match read_mixed() {
            Some(key) => key,
            None => {
                thread::switch();
                self.read() // TODO#? block thread for now until we read something
            }
        };

        // TODO support del key
        match key {
            DecodedKey::Unicode('\n') => {
                print!("\n");
                // TODO Send parse command to parser
            }
            DecodedKey::Unicode('\x08') => {
                print!("\x1b[1D \x1b[1D");
                // TODO Pop from parser
            }
            DecodedKey::Unicode(ch) => {
                print!("{}", ch);
                // TODO Push to parser
            }
            // TODO handle raw keys
            _ => {}
        };

        key
    }
}
