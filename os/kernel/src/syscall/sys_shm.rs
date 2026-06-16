/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_shm                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ All systemcalls related to the shared memory.                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Laurenz Maslo, Univ. Duesseldorf, 26.01.2026                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use core::slice::from_raw_parts;
use alloc::string::{String, ToString};
use crate::{memory::shm};

pub extern "sysv64" fn sys_shm_open(path: *const u8, length: usize, size: usize, create: usize) -> isize {
    let bytes = unsafe { from_raw_parts(path, length) };
    let path_str = String::from_utf8_lossy(bytes).to_string();

    return shm::open(path_str, size, create == 1);
}

pub extern "sysv64" fn sys_shm_attach(shm_id:usize, read_only: usize) -> isize {
    return shm::attach(shm_id, read_only == 1);
}

pub extern "sysv64" fn sys_shm_detach(shm_ptr:*mut u8) -> isize {
    return shm::detach(shm_ptr)
}

pub extern "sysv64" fn sys_shm_unlink(path: *const u8, length: usize) -> isize {
    let bytes = unsafe { from_raw_parts(path, length) };
    let path_str = String::from_utf8_lossy(bytes).to_string();

    return shm::unlink(path_str)
}