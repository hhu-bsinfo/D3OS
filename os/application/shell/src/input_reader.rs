use concurrent::thread;
use terminal::{DecodedKey, print, read::read_mixed};

use crate::{executor::executor::Executor, parser::parser::Parser};

pub struct InputReader {}

impl InputReader {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn read(&self, parser: &mut impl Parser, executor: &Executor) -> DecodedKey {
        let key = match read_mixed() {
            Some(key) => key,
            None => {
                thread::switch();
                self.read(parser, executor) // TODO#? block thread for now until we read something
            }
        };

        // TODO support del key
        match key {
            DecodedKey::Unicode('\n') => {
                print!("\n");
                let command_line = parser.parse();
                executor.execute(command_line);
                parser.reset();
                // TODO#? Remember line here and add to history???
            }
            DecodedKey::Unicode('\x08') => {
                print!("\x1b[1D \x1b[1D");
                parser.pop();
            }
            DecodedKey::Unicode(ch) => {
                print!("{}", ch);
                parser.push(ch);
            }
            // TODO handle raw keys
            _ => {}
        };

        key
    }
}
