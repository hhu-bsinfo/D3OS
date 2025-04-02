#![no_std]

extern crate alloc;

use naming::{mkfifo, open, ROOT};
use naming::shared_types::{OpenOptions, SeekOrigin};
#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use time::systime;

#[unsafe(no_mangle)]
pub fn main() {
    let start_time = systime();
    println!("naming test: start");
    
    // opening file
    let res = naming::touch("file.txt", OpenOptions::all(), ROOT);
    if res.is_err() {
        println!("touch error = {:?}", res);
        return;
    }
    let file = res.unwrap();
    let res = open(file, OpenOptions::READWRITE);

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
    if res.is_ok() {
        let len = res.unwrap();
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
    
    let Ok(dir) = naming::mkdir("test", OpenOptions::all(), ROOT) else { 
        println!("mkdir error = {:?}", res);
        return 
    };
    println!("created dir '/test' = {:?}", res);
    
    let res = naming::mkdir("dir1", OpenOptions::all(), dir);
    println!("created dir '/test/dir1' = {:?}", res);
    
    let res = naming::mkdir("dir2", OpenOptions::all(), dir);
    println!("created dir '/test/dir2' = {:?}", res);
    
    let res = naming::touch("file1.txt", OpenOptions::all(), dir);
    println!("created file '/test/file1.txt' = {:?}", res);

    //No readdir as it would reveal all files in the directory
    //thus leak information about other processes' files

    println!("naming test: end");
    let end_time = systime();
    println!("time elapsed: {} ms", (end_time - start_time));
}
