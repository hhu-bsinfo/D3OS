use alloc::string::String;
use core::cmp::min;
use core::ffi::{c_char, c_int, c_size_t, VaList};
use core::ops::DerefMut;
use terminal::{print, println};
use terminal::write::TERMINAL_WRITER;
use crate::errno::errno::{set_errno, Errno};
use crate::stdio::{stderr, stdout, FILE};
use crate::str_from_c_ptr;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn putchar(ch: c_int) -> c_int {
    let ch = char::from_u32(ch as u32).expect("putchar: Invalid character");
    print!("{}", ch);
    ch as c_int
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn puts(str: *const c_char) -> c_int {
    let str = str_from_c_ptr(str);
    println!("{}", str);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vfprintf(stream: *mut FILE, format: *const c_char, vlist: VaList) -> c_int {
    let stream = stream as usize;
    if stream != stdout as usize && stream != stderr as usize {
        todo!("vfprintf only supports printing to the terminal for now...")
    }

    let mut terminal = TERMINAL_WRITER.lock();

    unsafe {
        printf_compat::format(format, vlist, printf_compat::output::fmt_write(terminal.deref_mut()))
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsnprintf(buffer: *mut c_char, bufsz: c_size_t, format: *const c_char, vlist: VaList) -> c_int {
    unsafe {
        let mut string = String::new();
        let result = printf_compat::format(format, vlist, printf_compat::output::fmt_write(&mut string));

        // Workaround for Doom:
        // Doom uses the format string "STCFN.%3d" to search for STCFN033, STCFN034, etc.
        // However, the printf implementation produces "STCFN33", "STCFN34", etc.
        if string.starts_with("STCFN") {
            string.insert(5, '0');
        }

        let bytes = string.as_bytes();
        let copy_len = min(bufsz as usize - 1, bytes.len());
        bytes.as_ptr().copy_to_nonoverlapping(buffer as *mut u8, copy_len);
        buffer.add(copy_len).write(0); // Null-terminate

        result
    }
}