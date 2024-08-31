/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: name_service                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: API of name service.                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 30.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::sync::Arc;
use alloc::vec::Vec;
use syscall::return_vals::{SyscallResult,Errno};
use ::log::info;

use crate::naming::name_service_internal;
use crate::naming::name_service_internal::get_root_dir;
use crate::naming::stat::Stat;

pub type Result<T> = ::core::result::Result<T, Errno>;



// Wrapper function to convert Result<(), Errno> to SyscallResult
fn convert_result(result: core::result::Result<(), Errno>) -> SyscallResult {
    match result {
        Ok(()) => Ok(0), // Convert the success case to a meaningful u64, e.g., 0
        Err(e) => Err(e), // Forward the error directly
    }
}

///
/// Description:
///    Add an entry (with or without data)
///
/// Parameters: \
///   `path` path (must exist) \
///   `name` name for the new entry \
///   `content` data bytes
///
pub fn mkentry(path: &str, name: &str, content: Vec<u8>) -> SyscallResult {
    convert_result(get_root_dir().mkentry(path, name, content))
}


///
/// Description:
///    Add a directory. Creates all sub directories for the given path (if they do not exist already)
///
/// Parameters: \
///   `path` path to be created
///
pub fn mkdir(path: &str) -> SyscallResult {
    convert_result(get_root_dir().mkdir(path))
}

///
/// Description:
///    Get `stat` info for the given entry
///
/// Parameters: \
///   `path` path to entry
///
pub fn stat(path: &str) -> Result<Stat> {
    get_root_dir().stat(path)
}

///
/// Description:
///    Get container contents for the given entry
///
/// Parameters: \
///   `path` path to entry
///
pub fn cont(path: &str) -> Result<Vec<u8>> {
    get_root_dir().cont(path)
}

///
/// Description:
///    Get directory content for the given directory
///
/// Parameters: \
///   `path` path to directory
///
pub fn dir(path: &str) -> Result<Vec<Stat>> {
    get_root_dir().dir(path)
}

///
/// Description:
///    Rename a naming service entry.
///
/// Parameters: \
///   `path` path&entry name \
///   `new_name` new name
///
pub fn rename(path: &str, new_name: &str) -> SyscallResult {
    convert_result(get_root_dir().rename(path, new_name))
}

///
/// Description:
///    Rename a naming service entry.
///
/// Parameters: \
///   `path`  path&entry name
///
pub fn del(path: &str) -> SyscallResult {
    convert_result(get_root_dir().del(path))
}

///
/// Description:
///    Dump full name space
///
pub fn dump() {
    get_root_dir().dump(0);
}

///
/// Description:
///    Init naming service (called only once during booting)
///
pub fn init() {
    name_service_internal::NAME_SERVICE
        .call_once(|| Arc::new(name_service_internal::NameService::new()));
    info!("Initialized");
}

