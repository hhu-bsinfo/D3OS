/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_terminal                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for terminal.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 30.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::slice::from_raw_parts;
use core::slice::from_raw_parts_mut;
use core::sync::atomic::AtomicBool;
use log::error;
use syscall::return_vals::Errno;
use terminal::{TerminalInputState, TerminalMode};

use crate::device::tty::TtyInputState;
use crate::{tty_input, tty_output};

static KILL_OPERATOR_FLAG: AtomicBool = AtomicBool::new(false);

/// SystemCall implementation for SystemCall::TerminalWriteOutput.
/// Used by applications to write output in the terminal.
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

/// SystemCall implementation for SystemCall::TerminalReadOutput.
/// Used by terminal to read output from applications.
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

/// SystemCall implementation for SystemCall::TerminalWriteInput.
/// Used by terminal to write input for applications.
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

/// SystemCall implementation for SystemCall::TerminalReadInput.
/// Used by applications to read input from the terminal.
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

/// SystemCall implementation for SystemCall::TerminalCheckInputState.
/// Used by terminal to check if an applications is waiting for input.
///
/// Author: Sebastian Keller
pub fn sys_terminal_check_input_state() -> isize {
    let tty_input = tty_input();

    if tty_input.state() != TtyInputState::Waiting {
        return TerminalInputState::Idle as isize;
    }

    match tty_input.mode() {
        TerminalMode::Canonical => TerminalInputState::Canonical as isize,
        TerminalMode::Fluid => TerminalInputState::Fluid as isize,
        TerminalMode::Raw => TerminalInputState::Raw as isize,
    }
}
