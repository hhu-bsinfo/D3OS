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

use shared_types::{DirEntry, FileType, OpenOptions, RawDirent, SeekOrigin};
use syscall::{SystemCall, return_vals::Errno, syscall};



pub fn open(path: &str, flags: OpenOptions) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => {
            return syscall(SystemCall::Open, &[
                c_path.as_bytes().as_ptr() as usize,
                flags.bits(),
            ]);
        }
        Err(_) => Err(Errno::EBADSTR),
    }
}

pub fn write(fh: usize, buf: &[u8]) -> Result<usize, Errno> {
    return syscall(SystemCall::Write, &[fh, buf.as_ptr() as usize, buf.len()]);
}

pub fn read(fh: usize, buf: &mut [u8]) -> Result<usize, Errno> {
    return syscall(SystemCall::Read, &[
        fh,
        buf.as_mut_ptr() as usize,
        buf.len(),
    ]);
}

pub fn seek(fh: usize, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
    return syscall(SystemCall::Seek, &[fh, offset, origin.into()]);
}

pub fn close(fh: usize) -> Result<usize, Errno> {
    return syscall(SystemCall::Close, &[fh]);
}

pub fn mkdir(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => {
            return syscall(SystemCall::MkDir, &[c_path.as_bytes().as_ptr() as usize]);
        }
        Err(_) => Err(Errno::EBADSTR),
    }
}

pub fn touch(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => {
            return syscall(SystemCall::Touch, &[c_path.as_bytes().as_ptr() as usize]);
        }
        Err(_) => Err(Errno::EBADSTR),
    }
}

pub fn readdir(fh: usize) -> Result<Option<DirEntry>, Errno> {
    let mut raw_dirent = RawDirent::new();
    let ret = syscall(SystemCall::Readdir, &[
        fh,
        raw_dirent.as_mut_ptr() as usize,
        mem::size_of::<RawDirent>(),
    ]);
    match ret {
        Ok(rescode) => {
            if rescode == 0 {
                return Ok(None);
            } else {
                return Ok(DirEntry::from_dirent(&raw_dirent).clone());
            }
        }
        Err(e) => Err(e),
    }
}

impl DirEntry {
    pub fn from_dirent(dirent: &RawDirent) -> Option<Self> {
        // Convert d_type to a FileType enum
        let file_type = match dirent.d_type {
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
            return Ok(name);
        },
        Err(e) => Err(e),
    }
}

pub fn cd(path: &str) -> Result<usize, Errno> {
    match CString::new(path) {
        Ok(c_path) => {
            return syscall(SystemCall::Cd, &[c_path.as_bytes().as_ptr() as usize]);
        }
        Err(_) => Err(Errno::EBADSTR),
    }
}