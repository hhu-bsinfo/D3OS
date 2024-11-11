#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use concurrent::thread;
use terminal::{read::read, print, println, Application};
#[allow(unused_imports)]
use runtime::*;


fn process_next_char(line: &mut String, ch: char) {
    match ch {
        '\n' => {
            let split = line.split_whitespace().collect::<Vec<&str>>();
            if !split.is_empty() {
                match thread::start_application(split[0], split[1..].iter().map(|&s| s).collect()) {
                    Some(app) => app.join(),
                    None => println!("Command not found!"),
                }
            }

            line.clear();
            print!("> ");
        },
        '\x08' => { 
            line.pop(); 
        }, 
        _ => {
            line.push(ch);
        },
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut line = String::new();
    print!("> ");

    loop {
        match read(Application::Shell) {
            Some(ch) => process_next_char(&mut line, ch),
            None => (),
        }
    }
}
