/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_naming                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for the naming service.                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 30.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use syscall::return_vals::{convert_syscall_result_to_ret_code,SyscallResult,Errno};

use crate::naming::name_service;



pub fn sys_mkentry(
    path_buff: *const u8,
    path_buff_len: usize,
    name_buff: *const u8,
    name_buff_len: usize,
    data: usize,
) -> i64 {
    let path = from_utf8(unsafe {
        slice_from_raw_parts(path_buff, path_buff_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();
    let name = from_utf8(unsafe {
        slice_from_raw_parts(name_buff, name_buff_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();

    let r = name_service::mkentry(path, name, vec![1]);
    return convert_syscall_result_to_ret_code(r);
}

// Wrapper function to convert Result<(), Errno> to SyscallResult
fn convert_result(result: Result<(), Errno>) -> SyscallResult {
    match result {
        Ok(()) => Ok(0), // Convert the success case to a meaningful u64, e.g., 0
        Err(e) => Err(e), // Forward the error directly
    }
}
