/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_naming                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for the naming service.                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 28.12.2024, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::slice;
use alloc::string::{String, ToString};
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use core::mem;
use naming::shared_types::{OpenOptions, SeekOrigin, RawDirent};
use syscall::return_vals::{self, Errno};
use num_enum::FromPrimitive;

use crate::naming::api;

pub fn sys_open(path: *const u8, flags: OpenOptions) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::open(&ptr_to_string(path).unwrap(), flags))
}

pub fn sys_read(fh: usize, buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &mut[u8];
    unsafe {
        buf = slice::from_raw_parts_mut(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::read(fh, buf))
}

pub fn sys_write(fh: usize, buffer: *const u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &[u8];
    unsafe {
        buf = slice::from_raw_parts(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::write(fh, buf))
}

pub fn sys_seek(fh: usize, offset: usize, origin: usize) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::seek(fh, offset, SeekOrigin::from_primitive(origin)))
}

pub fn sys_close(fh: usize) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::close(fh))
}

pub fn sys_mkdir(path: *const u8) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::mkdir(&ptr_to_string(path).unwrap()))
}

pub fn sys_touch(path: *const u8) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::touch(&ptr_to_string(path).unwrap()))
}

/// Convert a raw pointer resulting from a CString to a UTF-8 String
fn ptr_to_string(ptr: *const u8) -> Result<String, Errno> {
    if ptr.is_null() {
        return Err(Errno::EBADSTR);
    }

    let mut len = 0;
    // Find the null terminator to determine the length
    unsafe {
        while *ptr.add(len) != 0 {
            len += 1;
        }
    }

    let path = from_utf8(unsafe { slice_from_raw_parts(ptr, len).as_ref().unwrap() });
    match path {
        Ok(path_str) => Ok(path_str.to_string()),
        Err(_) => Err(Errno::EBADSTR),
    }   
}

pub fn sys_readdir(fh: usize, buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 || buffer_length <  mem::size_of::<RawDirent>() {
        return Errno::EINVAL as isize;
    }
    let dentry = buffer as *mut RawDirent;
    return_vals::convert_syscall_result_to_ret_code(api::readdir(fh, dentry))
}


pub fn sys_cwd(buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &mut[u8];
    unsafe {
        buf = slice::from_raw_parts_mut(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::cwd(buf))
}

pub fn sys_cd(path: *const u8) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::cd(&ptr_to_string(path).unwrap()))
}
