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

#[cfg(feature = "userspace")]
use alloc::string::String;
#[cfg(feature = "userspace")]
use alloc::ffi::CString;
#[cfg(feature = "userspace")]
use core::mem;

#[cfg(feature = "userspace")]
use shared_types::{DirEntry, FileType, OpenOptions, RawDirent, SeekOrigin};
#[cfg(feature = "userspace")]
use syscall::{SystemCall, return_vals::Errno, syscall};


#[cfg(feature = "userspace")]
pub fn open(path: &str, flags: OpenOptions) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => syscall(SystemCall::Open, &[
            c_path.as_bytes().as_ptr() as usize,
            flags.bits(),
        ]),
        Err(_) => Err(Errno::EBADSTR),
    }
}

#[cfg(feature = "userspace")]
pub fn write(fh: usize, buf: &[u8]) -> Result<usize, Errno> {
    syscall(SystemCall::Write, &[fh, buf.as_ptr() as usize, buf.len()])
}

#[cfg(feature = "userspace")]
pub fn read(fh: usize, buf: &mut [u8]) -> Result<usize, Errno> {
    syscall(SystemCall::Read, &[
        fh,
        buf.as_mut_ptr() as usize,
        buf.len(),
    ])
}

#[cfg(feature = "userspace")]
pub fn seek(fh: usize, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
    syscall(SystemCall::Seek, &[fh, offset, origin.into()])
}

#[cfg(feature = "userspace")]
pub fn close(fh: usize) -> Result<usize, Errno> {
    syscall(SystemCall::Close, &[fh])
}

#[cfg(feature = "userspace")]
pub fn mkdir(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => syscall(SystemCall::MkDir, &[c_path.as_bytes().as_ptr() as usize]),
        Err(_) => Err(Errno::EBADSTR),
    }
}

#[cfg(feature = "userspace")]
pub fn touch(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => syscall(SystemCall::Touch, &[c_path.as_bytes().as_ptr() as usize]),
        Err(_) => Err(Errno::EBADSTR),
    }
}

#[cfg(feature = "userspace")]
pub fn readdir(fh: usize) -> Result<Option<DirEntry>, Errno> {
    let mut raw_dirent = RawDirent::new();
    let ret = syscall(SystemCall::Readdir, &[
        fh,
        raw_dirent.as_mut_ptr() as usize,
        mem::size_of::<RawDirent>(),
    ]);
    match ret {
        Ok(0) => Ok(None),
        Ok(_) => Ok(DirEntry::from_dirent(&raw_dirent).clone()),
        Err(e) => Err(e),
    }
}

#[cfg(feature = "userspace")]
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

#[cfg(feature = "userspace")]
pub fn cwd() -> Result<String, Errno> {
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

#[cfg(feature = "userspace")]
pub fn cd(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => syscall(SystemCall::Cd, &[c_path.as_bytes().as_ptr() as usize]),
        Err(_) => Err(Errno::EBADSTR),
    }
}

#[cfg(feature = "userspace")]
pub fn mkfifo(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => syscall(SystemCall::Mkfifo, &[c_path.as_bytes().as_ptr() as usize]),
        Err(_) => Err(Errno::EBADSTR),
    }
}
