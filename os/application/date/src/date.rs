#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use io::{print, println};
use time::date;

#[no_mangle]
pub fn main() {
    let date = date();
    println!("{}", date.format("%Y-%m-%d %H:%M:%S"));
}