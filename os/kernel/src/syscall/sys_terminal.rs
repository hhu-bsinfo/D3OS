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
use terminal::Application;
use stream::{InputStream};

use crate::{keyboard, terminal};

pub fn sys_terminal_read(application_ptr: *const Application, blocking: usize) -> isize {
    let enum_val = unsafe { application_ptr.as_ref().unwrap() };

    match enum_val {
        Application::Shell => {
            let terminal = terminal();
            match terminal.read_byte() {
                -1 => panic!("Input stream closed!"),
                c => c as isize
            };
        }
        Application::WindowManager => {
            if let Some(keyboard) = keyboard() {
                
                if blocking != 0 {
                    return keyboard.read_byte() as isize;
                }
                
                return keyboard.try_read_byte().unwrap_or(0) as isize;
            }
            return 0;
        }
    }

    let terminal = terminal();
    match terminal.read_byte() {
        -1 => panic!("Input stream closed!"),
        c => c as isize
    }
}

pub fn sys_terminal_write(buffer: *const u8, length: usize) -> isize {
    let string = from_utf8(unsafe { slice_from_raw_parts(buffer, length).as_ref().unwrap() }).unwrap();
    let terminal = terminal();
    terminal.write_str(string);
    0
}
