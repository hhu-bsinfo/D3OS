#![no_std]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use naming::cwd;

use naming::shared_types::{DirEntry, FileType, OpenOptions};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};

fn print_usage() {
    println!("usage: ls [directory_name");
}

fn print_dir_entry(dentry: DirEntry) {
    if dentry.file_type == FileType::Directory {
        println!("d {}", dentry.name);
    } else if dentry.file_type == FileType::NamedPipe {
        println!("p {}", dentry.name);
    } else {
        println!("- {}", dentry.name);
    }
}

fn process_ls(path: &str) {
    // open directory
    let res = naming::open(path, OpenOptions::DIRECTORY);
    if res.is_err() {
        print_usage();
        return;
    }
    let fd = res.unwrap();

    // dump content of directory
    loop {
        let res = naming::readdir(fd);
        match res {
            Ok(data) => {
                match data {
                    Some(content) => print_dir_entry(content),
                    None => break,
                }
            },
            Err(_) => break
        }
    }

    // close directory
    naming::close(fd).expect("Failed to close directory");
}

pub fn args_to_vec() -> Vec<String> {
    let args = env::args();
    let mut vec = Vec::new();
    for arg in args {
        vec.push(arg);
    }
    vec
}

#[unsafe(no_mangle)]
pub fn main() {
    let args_vec = args_to_vec();
    let args_count = args_vec.len();


    if args_count == 1 {
        let res = cwd();
        match res {
            Ok(path) =>  process_ls(&path),
            Err(_) => print_usage(),
        }
    } else if args_count == 2 {
        process_ls(&args_vec[1]);
    } else {
        print_usage();
    }
}
