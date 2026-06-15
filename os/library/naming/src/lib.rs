/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 28.12.2024, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

extern crate alloc;

#[macro_use]
extern crate bitflags;

pub mod shared_types;

use alloc::string::String;
use alloc::ffi::CString;
use core::mem;

use shared_types::{DirEntry, FileType, OpenOptions, RawDirent, SeekOrigin, Capability};
use syscall::{SystemCall, return_vals::Errno, syscall};
use terminal::print;

pub static ROOT : Capability = Capability::new(0);
pub static SHARED_PIPE : Capability = Capability::new(1);

/*pub fn root() -> Result<usize, Errno> {
    syscall(SystemCall::Root, &[])
}*/

pub fn open(cap_handle: Capability, flags: OpenOptions) -> Result<Capability, Errno> {
    match syscall(SystemCall::Open, &[
        cap_handle.handle(),
        flags.bits(),
    ]) {
        Ok(cap_handle) => Ok(Capability::new(cap_handle)),
        Err(e) => Err(e),
    }
}

pub fn write(cap: Capability, buf: &[u8]) -> Result<usize, Errno> {
    syscall(SystemCall::Write, &[cap.handle(), buf.as_ptr() as usize, buf.len()])
}

pub fn read(cap: Capability, buf: &mut [u8]) -> Result<usize, Errno> {
    syscall(SystemCall::Read, &[
        cap.handle(),
        buf.as_mut_ptr() as usize,
        buf.len(),
    ])
}

pub fn seek(cap: Capability, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
    syscall(SystemCall::Seek, &[cap.handle(), offset, origin.into()])
}

pub fn close(cap: Capability) -> Result<usize, Errno> {
    syscall(SystemCall::Close, &[cap.handle()])
}

pub fn mkdir(path: &str, flags: OpenOptions, dir_handle: Capability) -> Result<Capability, Errno> {
    match CString::new(path) {
        Ok(c_path) => {
            match syscall(SystemCall::MkDir, &[c_path.as_bytes().as_ptr() as usize,  flags.bits(), dir_handle.handle()]){
                Ok(cap_handle) => Ok(Capability::new(cap_handle)),
                Err(e) => Err(e),
            }
        },
        Err(_) => Err(Errno::EBADSTR),
    }
}

pub fn touch(path: &str, flags: OpenOptions, dir_handle: Capability) -> Result<Capability, Errno> {
    match CString::new(path) {
        Ok(c_path) => {
            match syscall(SystemCall::Touch, &[c_path.as_bytes().as_ptr() as usize,  flags.bits(), dir_handle.handle()]){
                Ok(cap_handle) => Ok(Capability::new(cap_handle)),
                Err(e) => Err(e),
            }
        },
        Err(_) => Err(Errno::EBADSTR),
    }
}

pub fn readdir(fh: Capability) -> Result<Option<DirEntry>, Errno> {
    return Err(Errno::ENOTSUP);
    let mut raw_dirent = RawDirent::new();
    let ret = syscall(SystemCall::Readdir, &[
        fh.handle(),
        raw_dirent.as_mut_ptr() as usize,
        size_of::<RawDirent>(),
    ]);
    match ret {
        Ok(0) => Ok(None),
        Ok(_) => Ok(DirEntry::from_dirent(&raw_dirent).clone()),
        Err(e) => Err(e),
    }
}

impl DirEntry {
    pub fn from_dirent(dirent: &RawDirent) -> Option<Self> {
        // Convert d_type to a FileType enum
        let file_type = match dirent.d_type {
            1 => FileType::NamedPipe,
            4 => FileType::Directory,
            8 => FileType::Regular,
            10 => FileType::Link,
            _ => return None, // Return None for unsupported file types
        };

        // Convert d_name (null-terminated) to a Rust String
        let name = dirent
            .d_name
            .iter()
            .take_while(|&&c| c != 0) // Stop at the null terminator
            .map(|&c| c as char)
            .collect::<String>();

        // If the name is empty, return None
        if name.is_empty() {
            return None;
        }

        Some(DirEntry { file_type, name })
    }
}

pub fn cwd() -> Result<String, Errno> {
    return Err(Errno::ENOTSUP);
    let buf: [u8; 512] = [0; 512]; // buffer for the path
    let result = syscall(SystemCall::Cwd, &[ buf.as_ptr() as usize, buf.len(), ]);
    match result {
        Ok(_) => {
            // Convert d_name (null-terminated) to a Rust String
            let name = buf
                .iter()
                .take_while(|&&c| c != 0) // Stop at the null terminator
                .map(|&c| c as char)
                .collect::<String>();
            Ok(name)
        },
        Err(e) => Err(e),
    }
}

pub fn cd(path: &str) -> Result<usize, Errno> {
    return Err(Errno::ENOTSUP);
    match CString::new(path) {
        Ok(c_path) => syscall(SystemCall::Cd, &[c_path.as_bytes().as_ptr() as usize]),
        Err(_) => Err(Errno::EBADSTR),
    }
}

pub fn mkfifo(name: &str, flags: OpenOptions, dir: Capability) -> Result<Capability, Errno> {
    // print!("lib::mkfifo called with path: {}, flags: {:?}, dir_handle: {} \n", path, flags, dir.handle());
    match CString::new(name) {
        Ok(c_path) => {
            match syscall(SystemCall::Mkfifo, &[c_path.as_bytes().as_ptr() as usize, flags.bits(), dir.handle()]){
                Ok(cap_handle) => Ok(Capability::new(cap_handle)),
                Err(e) => Err(e)
            }
        },
        Err(_) => Err(Errno::EBADSTR),
    }
}
