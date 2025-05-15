use alloc::string::{String, ToString};
/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: read                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Read a input char from terminal.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use syscall::{SystemCall, syscall};

use crate::{Application, TerminalMode};

pub fn try_read(application: Application) -> Option<char> {
    let application_addr = core::ptr::addr_of!(application) as usize;
    let res = syscall(SystemCall::TerminalReadInput, &[application_addr /*, 0*/]);

    match res {
        Ok(0) => None,
        Ok(ch) => Some(char::from_u32(ch as u32).unwrap()),
        Err(_) => None,
    }
}

pub fn read() -> String {
    let mut buffer: [u8; 128] = [0; 128];

    let read_bytes = syscall(
        SystemCall::TerminalReadInput,
        &[
            buffer.as_mut_ptr() as usize,
            buffer.len(),
            TerminalMode::Cooked as usize,
        ],
    )
    .expect("Unable to read input");

    String::from_utf8_lossy(&buffer[0..read_bytes]).to_string()
}
