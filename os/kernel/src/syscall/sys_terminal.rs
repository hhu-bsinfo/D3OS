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
use core::sync::atomic::{AtomicBool, Ordering};
use core::{ptr::slice_from_raw_parts, slice::from_raw_parts_mut};
use log::{error, info};
use syscall::return_vals::Errno;
use terminal::{TerminalInputState, TerminalMode};

use crate::device::tty::TtyInputState;
use crate::{tty_input, tty_output};

static KILL_OPERATOR_FLAG: AtomicBool = AtomicBool::new(false);

/// For applications
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_write_output(address: *const u8, length: usize) -> isize {
    if address.is_null() {
        error!("Output buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let bytes = unsafe { from_raw_parts(address, length) };
    tty_output().write(bytes) as isize
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_read_output(address: *mut u8, length: usize) -> isize {
    if address.is_null() {
        error!("Output buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let buffer = unsafe { from_raw_parts_mut(address, length) };
    tty_output().read(buffer) as isize
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_write_input(address: *mut u8, length: usize, mode: usize) -> isize {
    if address.is_null() {
        error!("Input buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let mode = TerminalMode::from(mode);
    let bytes = unsafe { from_raw_parts(address, length) };
    tty_input().write(bytes, mode) as isize
}

/// For Application
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_read_input(address: *mut u8, length: usize, mode: usize) -> isize {
    if address.is_null() {
        error!("Input buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let mode = TerminalMode::from(mode);
    let buffer = unsafe { from_raw_parts_mut(address, length) };
    tty_input().read(buffer, mode) as isize
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

/// For Application & Terminal
/// TODO#8 Do proper docs
///
/// Order the operator process (usually shell) to exit (Workaround due to missing ipc)
///
///
/// Author: Sebastian Keller
pub fn sys_terminal_terminate_operator(cmd: bool, ack: bool) -> isize {
    if cmd {
        KILL_OPERATOR_FLAG.store(true, Ordering::SeqCst);
        return 0;
    }

    if ack && KILL_OPERATOR_FLAG.load(Ordering::SeqCst) {
        KILL_OPERATOR_FLAG.store(false, Ordering::SeqCst);
        return 1;
    }

    0
}
