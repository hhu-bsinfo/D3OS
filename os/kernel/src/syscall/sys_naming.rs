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
use syscall::return_vals::{convert_syscall_result_to_ret_code};

use crate::naming::name_service;



pub fn sys_mkentry(
    path_buff: *const u8,
    path_buff_len: usize,
    name_buff: *const u8,
    name_buff_len: usize
) -> isize {
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
    convert_syscall_result_to_ret_code(r)
}
