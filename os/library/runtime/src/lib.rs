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
#[allow(unused_extern_crates)]
extern crate alloc;

#[cfg(feature="no-std")]
pub mod env;

#[cfg(feature="no-std")]
use concurrent::{process, thread};
#[cfg(feature="no-std")]
use core::panic::PanicInfo;
//#[cfg(feature="no-std")]
//use terminal::println;
//use linked_list_allocator::LockedHeap; (moved lower)
#[cfg(feature="no-std")]
use syscall::{syscall, SystemCall};

#[cfg(feature="no-std")]
unsafe extern "C" {
    fn main(argc: isize, argv: *const *const u8) -> isize;
}

cfg_if::cfg_if! {
	if #[cfg(feature="no-std")] {
		use linked_list_allocator::LockedHeap;
		#[global_allocator]
		static ALLOCATOR: LockedHeap = LockedHeap::empty(); 
	}
}

cfg_if::cfg_if! {
    if #[cfg(feature="no-std")] {
        #[panic_handler]
        fn panic(info: &PanicInfo<'_>) -> ! {
            //println!("Panic: {}!", info);
            thread::exit();
        }
    }
}

#[unsafe(no_mangle)]
#[cfg(feature="no-std")]
extern "sysv64" fn _start() {
    syscall(SystemCall::MapMemory, &[env::HEAP_START, env::HEAP_SIZE])
        .expect("Could not create user heap.");

    unsafe {
        ALLOCATOR.lock().init(env::HEAP_START as *mut u8, env::HEAP_SIZE);
    }

    thread::init_thread_environment(); //das ist neu

    unsafe {
        main(*env::ARGC_PTR as isize, env::ARGV_PTR);
    }
    process::exit();
}
