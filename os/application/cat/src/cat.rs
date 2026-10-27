#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use naming::cwd;

use naming::shared_types::OpenOptions;
use terminal::println;
use runtime::*;

fn print_usage() {
    println!("usage: cat file_name");
}

// Helperfunction for reading /proc/ps
/*fn read_proc_ps() -> Result<Vec<u8>, Errno> {
    let fd = open("/proc/ps", OpenOptions::READONLY)?;
    let mut out = Vec::new();
    let mut buf = [0u8; 512];

    loop {
        let n = read(fd, &mut buf)?;
        if n == 0 { break; }
        out.extend_from_slice(&buf[..n]);
    }

    close(fd)?;
    Ok(out)
}
*/

fn process_file(path_file: &str) {
    let res = naming::open(path_file, OpenOptions::READONLY);
    if res.is_err() {
        print_usage();
        return;
    }
    let fd = res.unwrap();

    loop {
        let mut buf = [0u8; 512];
        let res = naming::read(fd, &mut buf);
        match res {
            Ok(n) => {
                if n == 0 {
                    break;
                }
                let s = core::str::from_utf8(&buf[..n]).unwrap_or("[Invalid UTF-8]");
                println!("{}", s);
            }
            Err(_) => break,
        }
    }

    naming::close(fd).expect("Failed to close file");
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

    if args_count != 2 {
        print_usage();
    } else {
        let fname: &str = args_vec[1].as_str();

        if fname.starts_with('/') {
            process_file(fname);
        } else {
            let res = cwd();
            match res {
                Ok(path) => {
                    if path.len() == 1 && path.starts_with('/'){
                        let full_path: String = format!("{}{}", path, fname);
                        process_file(&full_path);
                    } else {
                        let full_path: String = format!("{}/{}", path, fname);
                        process_file(&full_path);
                    }
                }
                Err(e) => {
                    println!("Error: {:?}", e);
                    return;
                }
            }
        }
    }
}
