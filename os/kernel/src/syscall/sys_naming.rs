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

pub unsafe extern "sysv64" fn sys_open(path: *const u8, flag_bits: usize) -> isize {
    let flags = OpenOptions::from_bits(flag_bits).unwrap();
    return_vals::convert_syscall_result_to_ret_code(api::open(&unsafe { ptr_to_string(path).unwrap() }, flags))
}

pub unsafe extern "sysv64" fn sys_read(fh: usize, buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &mut[u8];
    unsafe {
        buf = slice::from_raw_parts_mut(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::read(fh, buf))
}

pub unsafe extern "sysv64" fn sys_write(fh: usize, buffer: *const u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &[u8];
    unsafe {
        buf = slice::from_raw_parts(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::write(fh, buf))
}

pub extern "sysv64" fn sys_seek(fh: usize, offset: usize, origin: usize) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::seek(fh, offset, SeekOrigin::from_primitive(origin)))
}

pub extern "sysv64" fn sys_close(fh: usize) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::close(fh))
}

pub unsafe extern "sysv64" fn sys_mkdir(path: *const u8) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::mkdir(&unsafe { ptr_to_string(path).unwrap() }))
}

pub unsafe extern "sysv64" fn sys_touch(path: *const u8) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::touch(&unsafe { ptr_to_string(path).unwrap() }))
}

/// Convert a raw pointer resulting from a CString to a UTF-8 String
unsafe fn ptr_to_string(ptr: *const u8) -> Result<String, Errno> {
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

pub unsafe extern "sysv64" fn sys_readdir(fh: usize, buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 || buffer_length <  mem::size_of::<RawDirent>() {
        return Errno::EINVAL as isize;
    }
    let dentry_ptr = buffer as *mut RawDirent;
    let dentry = unsafe { dentry_ptr.as_mut() };
    return_vals::convert_syscall_result_to_ret_code(api::readdir(fh, dentry))
}


pub unsafe extern "sysv64" fn sys_cwd(buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &mut[u8];
    unsafe {
        buf = slice::from_raw_parts_mut(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::cwd(buf))
}

pub unsafe extern "sysv64" fn sys_cd(path: *const u8) -> isize {
    return_vals::convert_syscall_result_to_ret_code(api::cd(&unsafe {ptr_to_string(path)}.unwrap()))
}
