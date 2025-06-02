#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use concurrent::thread;
use naming::{mkdir, touch, cwd, cd};
#[allow(unused_imports)]
use runtime::*;
use terminal::read::read;
use terminal::{print, println};


fn process_pwd(split: &[&str]) {
    if split.len() != 1 {
        println!("usage: pwd");
        return ;
    }
    let res = cwd();
    match res {
        Ok(path) =>  println!("{}", path),
        Err(_) => println!("usage: pwd"),
    }
}

fn process_mkdir(split: &[&str]) {
    if split.len() != 2 {
        println!("usage: mkdir directory_name");
        return ;
    }
    let res = mkdir(split[1]);
    if res.is_err() {
        println!("usage: mkdir directory_name");
    }
}

fn process_cd(split: &[&str]) {
    if split.len() != 2 {
        println!("usage: cd directory_name");
        return ;
    }
    let res = cd(split[1]);
    if res.is_err() {
        println!("usage: cd directory_name");
    }
}

fn process_internal_command(split: &Vec<&str>) -> bool {
    if split[0] == "pwd" {
        process_pwd(split);
        return true;
    } else if split[0] == "cd" {
        process_cd(split);
        return true;
    } else if split[0] == "mkdir" {
        process_mkdir(split);
        return true;
    } else if split[0] == "touch" {
        let res = touch(split[1]);
        if res.is_err() {
            println!("{:?}", res);
        }
        return true;
    }
    false
}

fn process_next_char(line: &mut String, ch: char) {
    match ch {
        '\n' => {
            let split = line.split_whitespace().collect::<Vec<&str>>();
            if !split.is_empty() {
                if !process_internal_command(&split) {
                    match thread::start_application(split[0], split[1..].iter().map(|&s| s).collect()) {
                        Some(app) => app.join(),
                        None => println!("Command not found!"),
                    }
                }
            }

            line.clear();
            print!("> ");
        }
        '\x08' => {
            line.pop();
        }
        _ => {
            line.push(ch);
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let mut line = String::new();
    print!("> ");

    loop {
        match read() {
            Some(ch) => process_next_char(&mut line, ch),
            None => (),
        }
    }
}
