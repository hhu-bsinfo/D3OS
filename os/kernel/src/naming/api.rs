/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: api                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Public interface of the name service (ns):                      ║
   ║         - mkdir: create a directory with all sub directories            ║
   ║         - open:  open a named object                                    ║
   ║         - read:  read bytes from an open object                         ║
   ║         - write: write bytes into an open object                        ║
   ║         - seek:  set file pointer (for files)                           ║
   ║         - init:  init ns, called once                                   ║
   ║         - dump:  print all entries on the screen (for debugging)        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 9.9.2024                 ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/


use alloc::string::String;
use syscall::return_vals::{OpenOptions, SeekOrigin, convert_syscall_result_to_ret_code};

use crate::naming::main;
use crate::naming::main::ns_get;
use crate::naming::open_objects::ns_get_oot;


///
/// Description:
///    Init function of NS. Must be called once before using it.
///
pub fn init() {
    main::init();
}

///
/// Description: Create a directory (including sub directories) for the given path
///
/// Parameters: `path` The path
///
/// Return: `SyscallResult`
///
pub fn mkdir(path: &String) -> isize {
    let result = ns_get().mkdir(path);
    let sysret;
    match result {
        Ok(()) => sysret = Ok(0),  // Convert the success case to a meaningful u64, e.g., 0
        Err(e) => sysret = Err(e), // Forward the error directly
    }
    return convert_syscall_result_to_ret_code(sysret);
}

///
/// Description: Open/create a named object
///
/// Parameters: \
///    `path` must be an absolute path \
///    `flags` see below
///
/// Return: `SyscallResult`
///
pub fn open(path: &String, flags: OpenOptions) -> isize {
    let res = ns_get().open(path, flags);
    let sysret;
    match res {
        Ok(fptr) => {
            sysret = ns_get_oot().lock().create_new_handle_for_filepointer(fptr);
        },
        Err(e) => sysret = Err(e),
    }
    return convert_syscall_result_to_ret_code(sysret);

/*    match res {
        Ok(fptr) => ns_get_oot().lock().create_new_handle_for_filepointer(fptr),
        Err(e) => Err(e),
    }
    */
}

///
/// Description: \
///    Write bytes from the given buffer into the file (at the current position). \
///    The number of bytes to be written is determined by the buffer size
///
/// Parameters: \
///    `fh`  file handle \
///    `buf` buffer from which bytes are copied into the file \
///
/// Return: `Ok(#bytes written)` or `Err(Errno)`
///
pub fn write(fh: usize, buf: &[u8]) -> isize {
    let sysret;
    match ns_get_oot().lock().get(fh) {
        Ok(fptr) => sysret = fptr.write(buf),
        Err(e) => sysret = Err(e),
    }
    return convert_syscall_result_to_ret_code(sysret);
}

///
/// Description: Set file pointer.
///
/// Parameters: \
///    `fh`  file handle \
///    `offset` offset in bytes \
///    `origin` point of origin
///
/// Return: `Ok(size in bytes)` or `Err(errno)`
///
pub fn seek(fh: usize, offset: usize, origin: SeekOrigin) -> isize {
    let sysret;
    match ns_get_oot().lock().get(fh) {
        Ok(fptr) => sysret = fptr.seek(offset, origin),
        Err(e) => sysret = Err(e),
    }
    return convert_syscall_result_to_ret_code(sysret);
}

///
/// Description: \
///    Read bytes from the file (from current position) into the given buffer. \
///    The number of bytes to be read is determined by the buffer size
///
/// Parameters: \
///    `fh`  file handle \
///    `buf` buffer to copy file bytes into \
///
/// Return: `Ok(#bytes read)` or `Err(errno)`
///
pub fn read(fh: usize, buf: &mut [u8]) -> isize {
    let sysret;
    match ns_get_oot().lock().get(fh) {
        Ok(fptr) => sysret = fptr.read(buf),
        Err(e) => sysret = Err(e),
    }
    return convert_syscall_result_to_ret_code(sysret);
}

///
/// Description: Dump all named objects on the screen (for debugging)
///
/// Return: `Ok(0)` or `Err(errno)`
///
pub fn dump() -> i64 { 
    ns_get().dump();
    return 0;
}