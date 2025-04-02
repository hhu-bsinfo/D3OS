/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_naming                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls for the naming service.                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 25.08.2025, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::slice;
use alloc::string::{String, ToString};
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use core::mem;
use log::{error, info, warn};
use naming::shared_types::{OpenOptions, SeekOrigin, RawDirent};
use syscall::return_vals::{self, Errno};
use num_enum::FromPrimitive;
use crate::capabilities::capability::Capability;
use crate::capabilities::capability_objects::naming_object::NamingObject;
use crate::naming::api;
use crate::scheduler;
use crate::syscall::syscall_dispatcher::init;
/*pub unsafe extern "sysv64" fn sys_open(path: *const u8, flag_bits: usize) -> isize {
    let flags = OpenOptions::from_bits(flag_bits).unwrap();
    return_vals::convert_syscall_result_to_ret_code(api::open(&unsafe { ptr_to_string(path).unwrap() }, flags))
}*/

/*pub unsafe extern "sysv64" fn sys_root(flag_bits: usize) -> isize {
    let flags = OpenOptions::from_bits(flag_bits).unwrap();

    return match api::root() {
        Ok(cap) => {
            // Store capability in current thread's CSpace
            if let Some(mut cspace) = scheduler().current_thread().cspace.invoke() {
                // Store the capability and return its handle
                // Implementation depends on your CSpace management
                return cspace.receive_root_naming_capability(Some(cap));;
            }
            Errno::EACCES as isize
        }
        Err(errno) => errno as isize
    }
}
*/
pub extern "sysv64" fn sys_open(cap_handle: usize, flag_bits: usize) -> isize {
    let current_thread = scheduler().current_thread();
    let flags = OpenOptions::from_bits(flag_bits).unwrap();
    let mut cspace = current_thread.cspace.invoke().unwrap();
    let naming_cap = cspace.get_naming_capability(cap_handle);

    if let Some(cap) = naming_cap {
        match api::open(flags, &cap) {
            Ok(cap) => {
                // info!("sys_open succeeded, storing new capability");
                    // Store the capability and return its handle
                let handle = cspace.receive_open_naming_capability(Some(cap));
                return handle;
                error!("Could not store cap");
                return Errno::EUNKN as isize; //Return EUNKN so that client doesnt know if it failed or if it existed
            }
            Err(errno) => {
                error!("sys_open failed: {:?}", errno);
                return Errno::EUNKN as isize; //Return EUNKN so that client doesnt know if it failed or if it existed
            }
        }
    }
    
    Errno::EUNKN as isize //Return EUNKN so that client doesnt know if it failed or if it existed
}


pub unsafe extern "sysv64" fn sys_read(cap_handle: usize, buffer: *mut u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }

    let current_thread = scheduler().current_thread();

    // Get the naming capability while holding the cspace lock
    let cspace = current_thread.cspace.invoke().unwrap();
    let open_naming_cap = cspace.get_open_naming_capability(cap_handle);

    // Now we can safely drop the cspace lock and proceed with the read operation
    if let Some(cap) = open_naming_cap {
        let buf = unsafe { slice::from_raw_parts_mut(buffer, buffer_length) };
        return return_vals::convert_syscall_result_to_ret_code(api::read(&cap, buf));
    }

    Errno::EACCES as isize
}


/*pub unsafe extern "sysv64" fn sys_write(fh: usize, buffer: *const u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }
    let buf: &[u8];
    unsafe {
        buf = slice::from_raw_parts(buffer, buffer_length);
    }
    return_vals::convert_syscall_result_to_ret_code(api::write(fh, buf))
}*/

pub unsafe extern "sysv64" fn sys_write(cap_handle: usize, buffer: *const u8, buffer_length: usize) -> isize {
    if buffer.is_null() || buffer_length == 0 {
        return Errno::EINVAL as isize;
    }

    let current_thread = scheduler().current_thread();
    let cspace = current_thread.cspace.invoke().expect("cspace locked");
    let open_naming_cap = cspace.get_open_naming_capability(cap_handle);

    // Now we can safely drop the cspace lock and proceed with the read operation
    if let Some(cap) = open_naming_cap {
        let buf = unsafe { slice::from_raw_parts(buffer, buffer_length) };
        return return_vals::convert_syscall_result_to_ret_code(api::write(&cap, buf));
    }

    Errno::EACCES as isize
}

pub extern "sysv64" fn sys_seek(cap_handle: usize, offset: usize, origin: usize) -> isize {
    let current_thread = scheduler().current_thread();
    let cspace = current_thread.cspace.invoke().unwrap();
    let naming_cap = cspace.get_open_naming_capability(cap_handle);

    if let Some(cap) = naming_cap {
        return return_vals::convert_syscall_result_to_ret_code(api::seek(&cap, offset, SeekOrigin::from_primitive(origin)));
    }

    Errno::EACCES as isize
}

pub extern "sysv64" fn sys_close(cap_handle: usize) -> isize {
    let current_thread = scheduler().current_thread();
    let mut cspace = current_thread.cspace.invoke().unwrap();
    let Some(cap) = cspace.get_open_naming_capability(cap_handle) else {
        error!("sys_close: cap not found for cap_handle: {}", cap_handle);
        return Errno::EACCES as isize;
    };


    match api::close(&cap) {
        Ok(_) => {
            // Remove the capability from the current thread's CSpace
            cspace.close_open_naming_capability(cap_handle);
            0
        },
        Err(errno) => errno as isize,
    }

}

pub unsafe extern "sysv64" fn sys_mkdir(name: *const u8, flag_bits: usize, dir_cap_handle: usize) -> isize {
    let current_thread = scheduler().current_thread();
    let flags = OpenOptions::from_bits(flag_bits).unwrap();
    let name = unsafe { ptr_to_string(name).unwrap() };

    // Get the capability and release the cspace lock before api call
    let mut cspace = current_thread.cspace.invoke().unwrap();
    let Some(dir_cap)= cspace.get_naming_capability(dir_cap_handle) else { 
        error!("sys_mkdir: cap not found for name: {}, dir_cap_handle: {}", name, dir_cap_handle);
        return Errno::EACCES as isize 
    };
    
    let path = dir_cap.invoke().unwrap().path.clone();

    match api::mkdir(&*(path + name.as_str()), flags, dir_cap) {
        Ok(cap) => {
            // Store capability in current thread's CSpace
            cspace.receive_naming_capability(Some(cap))
        }
        Err(errno) => errno as isize
    }
}

pub unsafe extern "sysv64" fn sys_touch(path: *const u8, flag_bits: usize ,cap_handle: usize) -> isize {
    let current_thread = scheduler().current_thread();
    let path = unsafe { ptr_to_string(path).unwrap() };
    let flags = OpenOptions::from_bits(flag_bits).unwrap();
    let mut cspace = current_thread.cspace.invoke().unwrap();
    let Some(naming_cap) = cspace.get_naming_capability(cap_handle) else {
        error!("sys_touch: cap not found for path: {}, cap_handle: {}", path, cap_handle);
        return Errno::EACCES as isize
    };


    match api::touch(&*path, flags, naming_cap) {
        Ok(cap) => {
            // Store capability in current thread's CSpace
            let handle = cspace.receive_naming_capability(Some(cap));
            handle
        },
        Err(_) => Errno::EINVAL as isize,
    }
}

pub unsafe extern "sysv64" fn sys_mkfifo(path: *const u8, flag_bits: usize, dir_cap_handle: usize) -> isize {
    let current_thread = scheduler().current_thread();
    let flags = OpenOptions::from_bits(flag_bits).unwrap();
    let path = unsafe { ptr_to_string(path).unwrap() };

    // info!("sys_mkfifo called with path: {}, flags: {:?}, dir_cap_handle: {}", path, flags, dir_cap_handle);
    
    // Get the capability and release the cspace lock before api call
    let mut cspace = current_thread.cspace.invoke().unwrap();
    let Some(dir_cap)= cspace.get_naming_capability(dir_cap_handle) else { return Errno::EACCES as isize };

    // Now make the api call with no locks held
    match api::mkfifo(&*path, flags, &dir_cap) {
        Ok(new_cap) => {
            // Reacquire the lock to store the new capability
            // info!("mkfifo succeeded, storing new capability");
            cspace.receive_naming_capability(Some(new_cap))
        }
        Err(errno) => {
            warn!("mkfifo failed");
            errno as isize
        }
    }
}

    /// Convert a raw pointer resulting from a CString to a UTF-8 String
pub(super) unsafe fn ptr_to_string(ptr: *const u8) -> Result<String, Errno> {
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

pub unsafe extern "sysv64" fn sys_readdir(cap_handle: usize, buffer: *mut u8, buffer_length: usize) -> isize {
    // if buffer.is_null() || buffer_length == 0 || buffer_length <  size_of::<RawDirent>() {
    //     return Errno::EINVAL as isize;
    // }
    // let current_thread = scheduler().current_thread();
    // let mut cspace = current_thread.cspace.invoke().unwrap();
    // let Some(dir_cap)= cspace.get_naming_capability(cap_handle) else { return Errno::EACCES as isize };
    // 
    // let path = dir_cap.invoke().unwrap().path.clone();

    Errno::EACCES as isize //Not implemented. Security risk. Should only read files from stored capabilities
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