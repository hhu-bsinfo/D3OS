/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: read                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Read a input char from terminal.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::ptr;
use syscall::{syscall, SystemCall};

pub fn read() -> Option<char> {
    let res = syscall(SystemCall::TerminalRead, &[]);
    match res {
        Ok(ch) => Some(char::from_u32(ch as u32).unwrap()),
        Err(_) => None,
    }
}

pub fn read_nb() -> Option<char> {
    let mut value: Option<i16> = None;
    let _ = syscall(SystemCall::TerminalReadNonBlocking, &[ptr::from_mut(&mut value) as usize]);

    match value {
        Some(-1) => None,
        Some(ch) => Some(char::from_u32(ch as u32).unwrap()),
        None => None,
    }
}