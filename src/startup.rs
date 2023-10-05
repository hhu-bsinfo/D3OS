#![feature(ptr_internals)]
#![feature(const_mut_refs)]
#![allow(dead_code)]          // avoid warnings 
#![allow(unused_variables)]   // avoid warnings 
#![allow(unused_imports)]
#![allow(unused_macros)]
#![feature(restricted_std)]

extern crate spin; // we need a mutex in devices::cga_print
extern crate std; // standard lib
extern crate tinyrlibc; // ensure we have 'strlen', needed to build 'std'
extern crate rlibc; // ensure we have compiler-builtin-funcs, needed to build 'std'


// insert other modules
#[macro_use]   // import macros, too
mod devices;
mod kernel;
mod user;
mod consts;

use devices::cga;         // shortcut for cga
use devices::cga_print;   // used to import code needed by println! 
use devices::keyboard;    // shortcut for keyboard

use kernel::cpu;

use user::aufgabe1::text_demo;
use user::aufgabe1::keyboard_demo;


fn aufgabe1() {
   cga::clear();
   text_demo::run();
   keyboard_demo::run();
}


#[no_mangle]
pub extern fn startup() {


	//cga::clear();
	


    aufgabe1();
    
    loop{}
}

