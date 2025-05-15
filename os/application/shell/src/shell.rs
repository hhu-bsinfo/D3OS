#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use concurrent::thread;
use naming::{cd, cwd, mkdir, touch};
#[allow(unused_imports)]
use runtime::*;
use terminal::read::read;
use terminal::{Application, print, println};

fn process_pwd(split: &Vec<&str>) {
    if split.len() != 1 {
        println!("usage: pwd");
        return;
    }
    let res = cwd();
    match res {
        Ok(path) => println!("{}", path),
        Err(_) => println!("usage: pwd"),
    }
}

fn process_mkdir(split: &Vec<&str>) {
    if split.len() != 2 {
        println!("usage: mkdir directory_name");
        return;
    }
    let res = mkdir(&split[1]);
    if res.is_err() {
        println!("usage: mkdir directory_name");
    }
}

fn process_cd(split: &Vec<&str>) {
    if split.len() != 2 {
        println!("usage: cd directory_name");
        return;
    }
    let res = cd(&split[1]);
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
        let res = touch(&split[1]);
        if res.is_err() {
            println!("{:?}", res);
        }
        return true;
    }
    return false;
}

fn process_line(line: String) {
    if line.is_empty() {
        return;
    }

    let split = line.split_whitespace().collect::<Vec<&str>>();
    if !split.is_empty() {
        if !process_internal_command(&split) {
            match thread::start_application(split[0], split[1..].iter().map(|&s| s).collect()) {
                Some(app) => app.join(),
                None => println!("Command not found!"),
            }
        }
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    loop {
        print!("> ");
        let line = read();
        process_line(line);
    }
}
