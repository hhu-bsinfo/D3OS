/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_terminal                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for terminal.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 30.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::slice::from_raw_parts;
use core::str::from_utf8;
use core::{ptr::slice_from_raw_parts, slice::from_raw_parts_mut};
use log::{debug, error};
use syscall::return_vals::Errno;
use terminal::{TerminalInputState, TerminalMode};

use crate::device::tty::TtyInputState;
use crate::{tty_input, tty_output};

/// For applications
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_write_output(address: *const u8, length: usize) -> isize {
    let tty_output = tty_output();
    let mut tty_output = tty_output.lock();

    if address.is_null() {
        error!("Write buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let bytes = unsafe { from_raw_parts(address, length) };

    tty_output.write(bytes) as isize
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_read_output(address: *mut u8, length: usize) -> isize {
    let tty_output = tty_output();
    let mut tty_output = tty_output.lock();

    if address.is_null() {
        error!("Write buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let buffer = unsafe { from_raw_parts_mut(address, length) };

    tty_output.read(buffer) as isize
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_check_input_state() -> isize {
    let tty_input = tty_input();

    if tty_input.state() != TtyInputState::Waiting {
        return TerminalInputState::Idle as isize;
    }

    match tty_input.mode() {
        TerminalMode::Cooked => TerminalInputState::InputReaderAwaitsCooked as isize,
        TerminalMode::Mixed => TerminalInputState::InputReaderAwaitsMixed as isize,
        TerminalMode::Raw => TerminalInputState::InputReaderAwaitsRaw as isize,
    }
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_write_input(address: *mut u8, length: usize, mode: usize) -> isize {
    let mode = TerminalMode::from(mode);
    let tty_input = tty_input();

    let bytes = unsafe { from_raw_parts(address, length) };
    tty_input.write(bytes, mode) as isize
}

/// For Application
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_read_input(address: *mut u8, length: usize, mode: usize) -> isize {
    let mode = TerminalMode::from(mode);
    let tty_input = tty_input();

    let buffer = unsafe { from_raw_parts_mut(address, length) };
    tty_input.read(buffer, mode) as isize
}

pub fn sys_log_debug(string_addr: *const u8, string_len: usize) {
    let log_string = from_utf8(unsafe {
        slice_from_raw_parts(string_addr, string_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();

    debug!("{}", log_string);
}
