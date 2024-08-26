/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 22.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

pub mod consts;

use syscall::{syscall5, SystemCall};

pub fn mkentry(path: &str, name: &str, data: usize) -> usize {
    let result = syscall5(
        SystemCall::Mkentry,
        path.as_bytes().as_ptr() as usize,
        path.len(),
        name.as_bytes().as_ptr() as usize,
        name.len(),
        data, // place holder, to be replaced by pointer to container
    );
/*
    let (code, val) = syscall5(call, arg1, arg2, arg3, arg4, arg5);

    let v = convert_codes_to_result(
        code,
        val,
        |c, _| c != 0,
        |_, _| unsafe { clock_info.assume_init() },
        |_, v| v.into(),
    );
*/
    result
}
