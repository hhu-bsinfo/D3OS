/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_naming                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for the naming service.                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 15.9.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use alloc::string::ToString;

use crate::naming::api;

pub fn sys_mkdir(path_buff: *const u8, path_buff_len: usize) -> isize {
    let path = from_utf8(unsafe {
        slice_from_raw_parts(path_buff, path_buff_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();

    api::mkdir(&path.to_string())
}
