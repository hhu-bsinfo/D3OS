#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use terminal::{print, println};
use time::date;
use concurrent::thread;

#[unsafe(no_mangle)]
pub fn main() {
   loop{
       let date = date();
       println!("{}", date.format("%Y-%m-%d %H:%M:%S"));
       thread::switch();
   }

}