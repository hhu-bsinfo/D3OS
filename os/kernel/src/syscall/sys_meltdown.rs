/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls (starting with sys_).                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use core::slice;
use alloc::boxed::Box;
use alloc::string::String;
use crate::scheduler;
use signal::signal_handler::SignalHandler;
use signal::signal_vector::SignalVector;
use log::{debug, info, warn};

pub fn sys_meltdown_copy_to_kernel_memory(string_content: *mut u8, string_len: usize) -> *const u8 {
    let string = unsafe {
        Box::new(String::from_raw_parts(string_content, string_len, string_len))
    };
    
    info!("Copying {} to kernel memory", string);
    let string_clone = Box::new(string.clone());
    let string_clone_ptr = string_clone.as_ptr();
    
    debug!("received string_content at {:p}, constructed old_address: {:p}, ptr: {:p}, new_address: {:p}, ptr: {:p}", string_content, &string, string.as_ptr(), &string_clone, string_clone_ptr);
    
    Box::leak(string); // Prevent calling drop on string, which is user memory and needs to be dropped by userspace
    Box::leak(string_clone); // We also want the box contents to live on for accessing it from userspace
    return string_clone_ptr as *const u8;
}
