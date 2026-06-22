#![no_std]

extern crate alloc;

use naming::ROOT;
use naming::shared_types::{OpenOptions, SeekOrigin};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};

#[unsafe(no_mangle)]
pub fn main() {
    println!("naming tests");

    // opening file
    let touch_res = naming::touch("file.txt", OpenOptions::all(), ROOT);
    if touch_res.is_err() {
        println!("touch error = {:?}", touch_res);
        return;
    }
    let file_cap = touch_res.unwrap();
    let res = naming::open(file_cap, OpenOptions::READWRITE | OpenOptions::CREATE);
    if res.is_err() {
        println!("open error = {:?}", res);
        return;
    }
    let fd = res.unwrap();

    // writing to file
    let buff = "Hello, World!".as_bytes();
    let res = naming::write(fd, buff);
    println!("write result = {:?}", res);

    // writing to file again
    let buff2 = " NRW Duesseldorf.".as_bytes();
    let res = naming::write(fd, buff2);
    println!("write result = {:?}", res);

    // seek to beginning
    let res = naming::seek(fd, 0, SeekOrigin::Start);
    println!("seek result = {:?}", res);

    // reading from file
    let mut rbuff: [u8; 512] = [0; 512];
    let res = naming::read(fd, &mut rbuff);
    println!("read result = {:?}", res);
    if let Ok(len) = res {
        for (i, byte) in rbuff.iter().enumerate() {
            if i >= len {
                break;
            }
            if byte.is_ascii_graphic() || *byte == b' ' {
                print!("{}", *byte as char);
            } else {
                print!(".");
            }
        }
    }
    println!("");

    let close_res = naming::close(fd);
    println!("close result = {:?}", close_res);

    let res = naming::mkdir("test", OpenOptions::all(), ROOT);
    println!("created dir '/test' = {:?}", res);
    let dir_cap = res.unwrap_or(ROOT);

    let res = naming::mkdir("dir1", OpenOptions::all(), dir_cap);
    println!("created dir '/test/dir1' = {:?}", res);

    let res = naming::mkdir("dir2", OpenOptions::all(), dir_cap);
    println!("created dir '/test/dir2' = {:?}", res);

    let res = naming::touch("file1.txt", OpenOptions::all(), dir_cap);
    println!("created file '/test/file1.txt' = {:?}", res);

    // opening directory
    let res = naming::open(dir_cap, OpenOptions::DIRECTORY);
    if res.is_err() {
        println!("open error = {:?}", res);
        return;
    }
    let fd = res.unwrap();
    println!("open dir '/test'");

    loop {
        let res = naming::readdir(fd);
        match res {
            Ok(data) => {
                match data {
                    Some(content) => println!("   readdir data = {:?}", content),
                    None => break,
                }
            },
            Err(_) => { 
                println!("   readdir failed");
                break;
            },
        }
    }

    let close_res = naming::close(fd);
    println!("close result = {:?}", close_res);

    println!("naming test: end");
}
