#![no_std]

extern crate alloc;

#[allow(unused_imports)]
use runtime::*;
use io::{print, println};
use time::systime;

#[no_mangle]
pub fn main() {
    let systime = systime();

    if systime.num_seconds() < 60 {
        println!("{}", systime.num_seconds());
    } else if systime.num_seconds() < 3600 {
        println!("{}:{:0>2}", systime.num_minutes(), systime.num_seconds() % 60);
    } else {
        let seconds = systime.num_seconds() - (systime.num_minutes() * 60);
        println!("{}:{:0>2}:{:0>2}", systime.num_hours(), systime.num_minutes() % 60, seconds);
    }
}