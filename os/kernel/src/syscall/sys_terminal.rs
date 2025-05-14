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
use terminal::TerminalInputState;

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
pub fn sys_terminal_input_state() -> isize {
    let tty_input = tty_input();
    let tty_input = tty_input.lock();

    if tty_input.can_write() {
        return TerminalInputState::InputReaderWaiting as isize;
    }

    TerminalInputState::Idle as isize
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_write_input(byte: u8) -> isize {
    let tty_input = tty_input();
    let mut tty_input = tty_input.lock();

    if !tty_input.can_write() {
        return Errno::EUNKN as isize;
    }

    tty_input.write(byte);
    0
}

/// For Application
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_read_input() -> isize {
    let tty_input = tty_input();
    let mut tty_input = tty_input.lock();

    tty_input.start_read();

    if !tty_input.can_read() {
        return 0;
    }

    let byte = tty_input.read();
    tty_input.end_read();

    byte as isize
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

// pub fn sys_terminal_read(application_ptr: *const Application, blocking: usize) -> isize {
//     let enum_val = unsafe { application_ptr.as_ref().unwrap() };

//     match enum_val {
//         Application::Shell => {
//             let terminal = terminal();
//             match terminal.read_byte() {
//                 -1 => panic!("Input stream closed!"),
//                 c => c as isize,
//             }
//         }
//         Application::WindowManager => {
//             if blocking != 0 {
//                 return keyboard()
//                     .expect("Failed to read from keyboard!")
//                     .decoded_read_byte() as isize;
//             }

//             return keyboard()
//                 .expect("Failed to read from keyboard!")
//                 .decoded_try_read_byte()
//                 .unwrap_or(0) as isize;
//         }
//     }
// }

// pub fn sys_terminal_write(buffer: *const u8, length: usize) -> isize {
//     let string =
//         from_utf8(unsafe { slice_from_raw_parts(buffer, length).as_ref().unwrap() }).unwrap();

//     // Prevent crashes when no terminal is available (window manager replaces the shell)
//     if terminal_initialized() {
//         let terminal = terminal();
//         terminal.write_str(string);
//     } else {
//         debug!("{}", string);
//     }

//     0
// }
