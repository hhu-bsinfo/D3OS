#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use naming::{mkdir, ROOT};
use naming::shared_types::OpenOptions;

#[unsafe(no_mangle)]
pub fn main() {
    let args = env::args();
    for (i, arg) in args.enumerate() {
        println!("Arg[{}]: {}", i, arg);
    }

    let Ok(res) = mkdir("home",OpenOptions::all(), ROOT) else { 
        println!("app: mkdir failed with error");
        return;
    };
    
    let res = mkdir("schoettner",OpenOptions::all(), res);

    println!("app: mkdir {:?}", res);
}