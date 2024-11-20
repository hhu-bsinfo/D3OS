#![no_std]

extern crate alloc;

use alloc::string::String;
use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use io::{print, println, Application};
use io::read::read;

#[no_mangle]
pub fn main() {
    let mut command = String::new();
    print!("> ");

    loop {
        match read(Application::Shell) {
            '\n' => {
                if !command.is_empty() {
                    match thread::start_application(command.as_str()) {
                        Some(app) => app.join(),
                        None => println!("Command not found!")
                    }
                }

                command.clear();
                print!("> ")
            },
            c => command.push(char::from_u32(c as u32).unwrap())
        }
    }
}