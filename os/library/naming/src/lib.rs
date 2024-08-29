/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 28.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

use syscall::{SystemCall, syscall, convert_syscall_codes_to_result};
use io::{print, println};

pub fn mkentry(path: &str, name: &str, data: usize) -> Result<usize, usize> {
    let (code, val) = syscall(
        SystemCall::Mkentry,
        &[
            path.as_bytes().as_ptr() as usize,
            path.len(),
            name.as_bytes().as_ptr() as usize,
            name.len(),
            data, // place holder, to be replaced by pointer to container
        ],
    );

    let v:Result<usize, usize> = convert_syscall_codes_to_result(
        code,
        val,
        |c, _| c != 0,
        |_, _| val.into(),
        |_, v| val.into(),
    );

    println!("lib/mkentry: result = {:?}", v);

    return v;
}
