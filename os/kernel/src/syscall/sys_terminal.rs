/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_terminal                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for terminal.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 30.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::tty;
use core::slice::from_raw_parts;
use core::str::from_utf8;
use core::{ptr::slice_from_raw_parts, slice::from_raw_parts_mut};
use log::{debug, error};
use syscall::return_vals::Errno;

/// For applications
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_write(address: *const u8, length: usize) -> isize {
    let tty = tty();
    let mut tty = tty.lock();

    if address.is_null() {
        error!("Write buffer must not be null");
        return Errno::EINVAL as isize;
    }

    let write = unsafe { from_raw_parts(address, length) };

    match tty.push_write(write) {
        Ok(_) => 0,
        Err(_) => Errno::EINVAL as isize,
    }
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_consume_write(address: *mut u8, length: usize) -> isize {
    let tty = tty();
    let mut tty = tty.lock();
    let tty_index = tty.write_index();

    if address.is_null() {
        error!("Write buffer must not be null");
        return Errno::EINVAL as isize;
    }

    if tty_index > length {
        error!(
            "Write buffer is to small (required: {}, received: {})",
            tty_index, length
        );
        return Errno::EINVAL as isize;
    };

    let write = tty.consume_write();
    let buffer = unsafe { from_raw_parts_mut(address, length) };
    buffer[..tty_index].copy_from_slice(&write[..tty_index]);

    0
}

/// For Terminal
/// TODO#8 Do proper docs
/// return 0 => is reading
/// return 1 => not reading
///
/// Author: Sebastian Keller
pub fn sys_terminal_can_produce_read() -> isize {
    let tty = tty();
    let tty = tty.lock();

    match tty.is_reading() {
        true => 0,
        false => 1,
    }
}

/// For Terminal
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_produce_read(byte: u8) -> isize {
    let tty = tty();
    let mut tty = tty.lock();

    if !tty.is_reading() {
        return 0;
    }

    tty.produce_read(byte);

    0
}

/// For Application
/// TODO#8 Do proper docs
///
/// Author: Sebastian Keller
pub fn sys_terminal_read() -> isize {
    let tty = tty();
    let mut tty = tty.lock();

    tty.start_reading();

    if !tty.can_read() {
        return 0;
    }

    let byte = tty.consume_read();
    tty.stop_reading();
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
