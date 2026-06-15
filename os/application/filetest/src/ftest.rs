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
    let Ok(file) = naming::touch("file.txt", OpenOptions::READWRITE | OpenOptions::CREATE, ROOT) else {
        println!("touch error");
        return;
    };

    let Ok(open_file) = naming::open(file, OpenOptions::READWRITE) else {
        println!("open error");
        return;
    };
    
    println!("open file '/file.txt', cap_handle = {:?}", file.handle());

    // writing to file
    let buff = "Hello, World!".as_bytes();
    let res = naming::write(open_file, buff);
    println!("write result = {:?}", res);

    // writing to file again
    let buff2 = " NRW Duesseldorf.".as_bytes();
    let res = naming::write(open_file, buff2);
    println!("write result = {:?}", res);

    // seek to beginning
    let res = naming::seek(open_file, 0, SeekOrigin::Start);
    println!("seek result = {:?}", res);

    // reading from file
    let mut rbuff: [u8; 512] = [0; 512];
    let res = naming::read(open_file, &mut rbuff);
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

    //let close_res = naming::close(cap_handle);
    //println!("close result = {:?}", close_res);

    let Ok(dir) = naming::mkdir("test", OpenOptions::all(), ROOT) else { 
        println!("mkdir error");
        return;
    };
    println!("created dir '/test' = {:?}", dir);

    let Ok(dir1) = naming::mkdir("dir1", OpenOptions::all(), dir) else { 
        println!("mkdir error");
        return;
    };
    println!("created dir '/test/dir1' = {:?}", dir1);

    let Ok(dir2) = naming::mkdir("dir2", OpenOptions::all(), dir) else { 
        println!("mkdir error");
        return;
    };
    println!("created dir '/test/dir1' = {:?}", dir2);

    let Ok(file1) = naming::touch("file1.txt", OpenOptions::all(), dir) else { 
        println!("touch error");
        return;
    };
    println!("created file '/test/file1.txt' = {:?}", file1);

    //no readdir

    println!("naming test: end");
}