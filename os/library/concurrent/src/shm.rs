/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: shm                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ All systemcalls for shared memory.                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Laurenz Maslo, Univ. Duesseldorf, 26.01.2026                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use syscall::{return_vals::Errno, syscall, SystemCall};


pub enum Permissions {
    READONLY  = 1,
    WRITEONLY = 2
}

impl Permissions {
    pub fn from(mode: usize) -> Option<Self> {
        match mode {
            1 => Some(Self::READONLY),
            2 => Some(Self::WRITEONLY),
            _ => None
        }
    }

    pub fn to(&self) -> usize {
        match *self {
            Self::READONLY => 1,
            Self::WRITEONLY => 2
        }
    }
}

/// Create or open a shared memory region with the given name and size. \
/// Returns an id for the shared memory region, which can be used to attach it into the virtual address space. 
pub fn shm_open(name: &str, size:usize, create: bool) -> Result<usize, Errno> {
    return syscall(SystemCall::ShmOpen, &[name.as_bytes().as_ptr() as usize, name.len(), size, create as usize]);
}

/// Attach the shared memory region with the given id into the virtual address space. \
/// Returns a pointer to the attached shared memory region.
pub fn shm_attach(shm_id:usize, read_only: bool) -> Result<*mut u8, Errno> {
    let res = syscall(SystemCall::ShmAttach, &[shm_id, read_only as usize]);
    match res {
        Ok(ptr) => Ok(ptr as *mut u8),
        Err(e) => Err(e)
    }
}

/// Detach the shared memory region at the given pointer from the virtual address space. \
/// The pointer must point to the start of the attached shared memory region.
pub fn shm_detach(ptr:*mut u8) -> Result<(), Errno> {
    let res = syscall(SystemCall::ShmDetach, &[ptr as usize]);
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e)
    }
}

/// Unlink the shared memory region with the given name. \
pub fn shm_unlink(name: &str) -> Result<(), Errno> {
    let res = syscall(SystemCall::ShmUnlink, &[name.as_bytes().as_ptr() as usize, name.len()]);
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(e)
    }
}