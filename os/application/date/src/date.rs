#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use time::date;

#[unsafe(no_mangle)]
pub fn main() {
    let date = date();
    println!("{}", date.format("%Y-%m-%d %H:%M:%S"));
}