/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_terminal                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for terminal.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 30.8.2024, HHU                                  ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use crate::{keyboard, terminal};
use log::debug;
use stream::{DecodedInputStream, InputStream};
use terminal::Application;

pub fn sys_log_debug(string_addr: *const u8, string_len: usize) {
    let log_string = from_utf8(unsafe {
        slice_from_raw_parts(string_addr, string_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();

    debug!("{}", log_string);
}

pub fn sys_terminal_read(application_ptr: *const Application, blocking: usize) -> isize {
    let enum_val = unsafe { application_ptr.as_ref().unwrap() };
    
    match enum_val {
        Application::Shell => {
            let terminal = terminal();
            match terminal.read_byte() {
                -1 => panic!("Input stream closed!"),
                c => c as isize
            }
        },
        Application::WindowManager => {
            if blocking != 0 {
                return keyboard().expect("Failed to read from keyboard!").decoded_read_byte() as isize;
            }

            return keyboard().expect("Failed to read from keyboard!").decoded_try_read_byte().unwrap_or(0) as isize;
        }
    }
}

pub fn sys_terminal_write(buffer: *const u8, length: usize) -> isize {
    let string = from_utf8(unsafe { slice_from_raw_parts(buffer, length).as_ref().unwrap() }).unwrap();
    let terminal = terminal();
    terminal.write_str(string);
    0
}
