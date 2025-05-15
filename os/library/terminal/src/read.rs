use alloc::string::{String, ToString};
use pc_keyboard::DecodedKey;
/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: read                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Read a input char from terminal.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 31.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use syscall::{SystemCall, syscall};

use crate::{Application, DecodedKeyType, TerminalMode};

pub fn try_read(application: Application) -> Option<char> {
    let application_addr = core::ptr::addr_of!(application) as usize;
    let res = syscall(SystemCall::TerminalReadInput, &[application_addr /*, 0*/]);

    match res {
        Ok(0) => None,
        Ok(ch) => Some(char::from_u32(ch as u32).unwrap()),
        Err(_) => None,
    }
}

/// TODO proper docs
/// Author: Sebastian Keller
///
/// Echoes written chars
/// Blocks until '\n'
/// Returns String
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

/// TODO proper docs
/// Author: Sebastian Keller
///
/// No echo
/// No blocking
/// Returns Option of DecodedKey (RawKey or Unicode)
pub fn read_mixed() -> Option<DecodedKey> {
    let mut buffer: [u8; 2] = [0; 2];

    let written_bytes = syscall(
        SystemCall::TerminalReadInput,
        &[
            buffer.as_mut_ptr() as usize,
            buffer.len(),
            TerminalMode::Mixed as usize,
        ],
    )
    .expect("Unable to read input");

    if written_bytes != 2 {
        return None;
    }

    let key_type = DecodedKeyType::from(*buffer.first().unwrap());
    let key = *buffer.last().unwrap();

    if key_type == DecodedKeyType::Unicode {
        return Some(DecodedKey::Unicode(key as char));
    }
    if key_type == DecodedKeyType::RawKey {
        return Some(DecodedKey::RawKey(unsafe { core::mem::transmute(key) }));
    }

    return None;
}

/// TODO proper docs
/// Author: Sebastian Keller
///
/// No echo
/// No blocking
/// Returns Option of raw undecoded u8
pub fn read_raw() -> Option<u8> {
    let mut buffer: [u8; 1] = [0; 1];

    syscall(
        SystemCall::TerminalReadInput,
        &[
            buffer.as_mut_ptr() as usize,
            buffer.len(),
            TerminalMode::Raw as usize,
        ],
    )
    .expect("Unable to read input");

    match *buffer.first().unwrap() {
        0 => None,
        byte => Some(byte),
    }
}
