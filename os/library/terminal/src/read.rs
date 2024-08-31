/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: read                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Read a input char from terminal.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use syscall::{syscall, SystemCall};

pub fn read() -> Option<char> {
    let res = syscall(SystemCall::TerminalRead, &[]);
    match res {
        Ok(ch) => Some(char::from_u32(ch as u32).unwrap()),
        Err(_) => None,
    }    
}