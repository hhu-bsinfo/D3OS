#![no_std]
extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
#[allow(unused_imports)]
use runtime::*;
use terminal::{println, read::read_fluid, DecodedKey};

#[unsafe(no_mangle)]
fn main() {
    let mut allocations = Vec::new();

    println!("heap test");
    println!("press A to allocate 25 kilobytes");
    println!("press D to deallocate 25 kilobytes");
    println!("press Q to exit");
    loop {
        println!("currently allocated: {} kilobytes ", allocations.len());
        let key = read_fluid();
        if let Some(DecodedKey::Unicode(key)) = key {
            match key {
                'a' | 'A' => for _ in 0..25 {
                    allocations.push(Box::new([0u8; 1024]));
                },
                'd' | 'D' => for _ in 0..25 {
                    allocations.pop();
                },
                'q' | 'Q' => break,
                _ => continue,
            }
        }
    }
}
