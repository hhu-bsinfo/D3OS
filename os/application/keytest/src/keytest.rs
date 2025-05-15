#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{
    DecodedKey, print, println,
    read::{read, read_mixed, read_raw},
};

#[unsafe(no_mangle)]
pub fn main() {
    let mut args = env::args();

    let arg = match args.nth(1) {
        Some(arg) => arg,
        None => {
            println!("Require terminal mode as argument (cooked | mixed | raw)");
            return;
        }
    };

    match arg.as_str() {
        "cooked" => {
            println!("Enter a line (type 'exit' to exit)");
            loop {
                print!("Echo: ");
                match read().as_str() {
                    "exit" => return,
                    line => println!("Read: {}", line),
                }
            }
        }
        "mixed" => {
            println!("Press any key (ESC to exit)");
            loop {
                loop {
                    match read_mixed() {
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
                        Some(1) => return,
                        Some(key) => println!("{}", key),
                        None => continue,
                    };
                    break;
                }
            }
        }
        _ => {
            println!("Invalid argument. Valid terminal modes are (cooked | mixed | raw)");
        }
    }
}
