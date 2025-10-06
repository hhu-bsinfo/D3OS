#![no_std]

extern crate alloc;

use pc_keyboard::{KeyEvent, KeyState};
#[allow(unused_imports)]
use runtime::*;
use terminal::{
    print, println, read::{read, read_fluid, read_raw}, DecodedKey, KeyCode
};

/// This application can be used to test the new terminal modes.
///
/// Author: Sebastian Keller
#[unsafe(no_mangle)]
pub fn main() {
    let mut args = env::args();

    let arg = match args.nth(1) {
        Some(arg) => arg,
        None => {
            println!("Require terminal mode as argument (canonical | fluid | raw)");
            return;
        }
    };

    match arg.as_str() {
        "canonical" => {
            println!("Enter a line (type 'exit' to exit)");
            loop {
                print!("Echo: ");
                match read().as_str() {
                    "exit" => return,
                    line => println!("Read: {}", line),
                }
            }
        }
        "fluid" => {
            println!("Press any key (ESC to exit)");
            loop {
                loop {
                    match read_fluid() {
                        Some(DecodedKey::Unicode('\u{1b}')) => return,
                        Some(key) => println!("{:?}", key),
                        None => continue,
                    };
                    break;
                }
            }
        }
        "raw" => {
            println!("Press any key (ESC to exit)");
            loop {
                loop {
                    match read_raw() {
                        Some(KeyEvent {
                            code: KeyCode::Escape, state: KeyState::Down,
                        }) => return,
                        Some(event) => println!("{:?}", event),
                        None => continue,
                    };
                    break;
                }
            }
        }
        _ => {
            println!("Invalid argument. Valid terminal modes are (canonical | fluid | raw)");
        }
    }
}
