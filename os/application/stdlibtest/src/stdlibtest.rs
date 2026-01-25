#[allow(unused_imports)]
use std::io::{self, Write};

#[unsafe(no_mangle)]
pub fn main() {
    //writing using stdout directly
    let stdout = io::stdout();
    let mut handle = stdout.lock();
    handle.write(b"Writing directly with stdout!\n");

    print!("This has been written using the print!-macro!\n");

    println!("And this has been written using println!!");
}
