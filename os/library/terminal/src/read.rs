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

use crate::{DecodedKeyType, TerminalMode};

/// Read from terminal in canonical mode.
///
/// The terminal will echo.
/// The application will block until 'Enter' is pressed.
/// Command line editing is enabled.
/// Returns written line.
///
/// Author: Sebastian Keller
pub fn read() -> String {
    let mut buffer: [u8; 128] = [0; 128];

    let read_bytes = syscall(
        SystemCall::TerminalReadInput,
        &[
            buffer.as_mut_ptr() as usize,
            buffer.len(),
            TerminalMode::Canonical as usize,
        ],
    )
    .expect("Unable to read input");

    String::from_utf8_lossy(&buffer[0..read_bytes]).to_string()
}

/// Read from terminal in fluid mode.
///
/// The terminal will not echo.
/// The application will not block.
/// Returns decoded key as well as raw special keys.
///
/// Author: Sebastian Keller
pub fn read_fluid() -> Option<DecodedKey> {
    let mut buffer: [u8; 2] = [0; 2];

    let written_bytes = syscall(
        SystemCall::TerminalReadInput,
        &[buffer.as_mut_ptr() as usize, buffer.len(), TerminalMode::Fluid as usize],
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

/// Read from terminal in raw mode.
///
/// The terminal will not echo.
/// The application will not block.
/// Returns raw undecoded key.
///
/// Author: Sebastian Keller
pub fn read_raw() -> Option<u8> {
    let mut buffer: [u8; 1] = [0; 1];

    syscall(
        SystemCall::TerminalReadInput,
        &[buffer.as_mut_ptr() as usize, buffer.len(), TerminalMode::Raw as usize],
    )
    .expect("Unable to read input");

    match *buffer.first().unwrap() {
        0 => None,
        byte => Some(byte),
    }
}
