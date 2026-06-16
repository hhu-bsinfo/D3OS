use core::ffi::{c_char, c_int, c_long, c_size_t, c_void};
use core::slice;
use naming::shared_types::{OpenOptions, SeekOrigin};
use crate::errno::errno::{set_errno, Errno};
use crate::stdio::FILE;
use crate::str_from_c_ptr;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fopen(filename: *const c_char, mode: *const c_char) -> *mut FILE {
    let mode = str_from_c_ptr(mode);
    if mode != "r" && mode != "rb" {
        todo!("libc only supports read-only fopen for now...");
    }

    let path = str_from_c_ptr(filename);
    match naming::open(path, OpenOptions::READONLY) {
        Ok(handle) => handle as *mut FILE,
        Err(_) => {
            set_errno(Errno::ENOENT);
            core::ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fclose(stream: *mut FILE) -> c_int {
    let handle = stream as usize;
    match naming::close(handle) {
        Ok(_) => 0,
        Err(_) => {
            set_errno(Errno::EBADF);
            1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fread(buffer: *mut c_void, size: c_size_t, count: c_size_t, stream: *mut FILE) -> c_size_t {
    let handle = stream as usize;
    let total_size = size * count;

    unsafe {
        let target = slice::from_raw_parts_mut(buffer as *mut u8, total_size as usize);

        match naming::read(handle, target) {
            Ok(bytes_read) => bytes_read / size,
            Err(_) => {
                set_errno(Errno::EBADF);
                0
            }
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fwrite(_buffer: *const c_void, _size: c_size_t, _count: c_size_t, _stream: *mut FILE) -> c_size_t {
    set_errno(Errno::EACCES);
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fflush(_stream: *mut FILE) -> c_int {
    0
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fseek(stream: *mut FILE, offset: c_long, origin: c_int) -> c_int {
    let handle = stream as usize;

    let mode = match origin {
        0 => SeekOrigin::Start,
        1 => SeekOrigin::Current,
        2 => SeekOrigin::End,
        _ => {
            set_errno(Errno::EDOM);
            return 1;
        }
    };

    match naming::seek(handle, offset as isize, mode) {
        Ok(_) => 0,
        Err(_) => {
            set_errno(Errno::EBADF);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn ftell(stream: *mut FILE) -> c_long {
    let handle = stream as usize;

    match naming::seek(handle, 0, SeekOrigin::Current) {
        Ok(position) => position as c_long,
        Err(_) => {
            set_errno(Errno::EBADF);
            -1
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn remove(_pathname: *const c_char) -> c_int {
    todo!()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn rename(_old_filename: *const c_char, _new_filename: *const c_char) -> c_int {
    todo!()
}