#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use io::{print, println};
use io::read::read;

#[unsafe(no_mangle)]
pub fn main() {
    let mut line = String::new();
    print!("> ");

    loop {
        match read() {
            '\n' => {
                let split = line.split_whitespace().collect::<Vec<&str>>();
                if !split.is_empty() {
                    match thread::start_application(split[0], split[1..].iter().map(|&s| s).collect()) {
                        Some(app) => app.join(),
                        None => println!("Command not found!")
                    }
                }

                line.clear();
                print!("> ")
            },
            c => line.push(char::from_u32(c as u32).unwrap())
        }
    }
}