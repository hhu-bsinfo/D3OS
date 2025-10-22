/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Entry function for an application.                              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

#![allow(unexpected_cfgs)]
#![no_std]

extern crate alloc;

pub mod env;

use concurrent::{process, thread};
use core::panic::PanicInfo;
use terminal::println;
use linked_list_allocator::LockedHeap;
use syscall::{syscall, SystemCall};

unsafe extern "C" {
    fn main(argc: isize, argv: *const *const u8) -> isize;
}

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

#[cfg(not(any(test, feature = "std")))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("Panic: {}!", info);
    thread::exit();
}

#[unsafe(no_mangle)]
extern "sysv64" fn entry() {
    // set up the thread environment, which is stored at FS:0
    thread::init_thread_environment();

    syscall(SystemCall::MapMemory, &[env::HEAP_START, env::HEAP_SIZE])
        .expect("Could not create user heap.");

    unsafe {
        ALLOCATOR.lock().init(env::HEAP_START as *mut u8, env::HEAP_SIZE);
    }

    unsafe {
        main(*env::ARGC_PTR as isize, env::ARGV_PTR);
    }
    process::exit();
}
