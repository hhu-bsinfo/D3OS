#![no_std]
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;
use naming::shared_types::OpenOptions;
#[allow(unused_imports)]
use runtime::*;
use terminal::println;



fn read_text_file(path: &str) -> Result<String, ()> {
    let fh = naming::open(path, OpenOptions::READONLY).map_err(|_| ())?;
    let mut out: Vec<u8> = Vec::new();
    let mut buf = [0u8; 512];

    loop {
        let n = naming::read(fh, &mut buf).map_err(|_| ())?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }

    let _ = naming::close(fh);
    let text = core::str::from_utf8(&out).map_err(|_| ())?;
    Ok(text.into())
}

fn print_section(title: &str, path: &str) {
    println!("== {} ==", title);
    match read_text_file(path) {
        Ok(text) => println!("{}", text),
        Err(_) => println!("unable to read {}", path),
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    println!("task_manager");
    print_section("Services", "/proc/services");
    print_section("Memory", "/proc/meminfo");
    print_section("Processes", "/proc/ps");
}