/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: shared_types                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Types used by the naming service both in user und kernel mode.  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 25.08.2025, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::string::String;
use num_enum::{FromPrimitive, IntoPrimitive};

bitflags! {
    /// Description: Option flags for opening objects
    pub struct OpenOptions: usize {
        const READONLY  = 1;
        const READWRITE = 2;
        const CREATE    = 3;
        const EXCLUSIVE = 4;
        const DIRECTORY = 5;
        const WRITEONLY = 6; // relevant for pipes
    }
}

/// Description: origin for `seek` 
#[derive(Debug, Copy, Clone, Eq, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum SeekOrigin {
    #[num_enum(default)]
    Start = 1,
    End = 2,
    Current = 3,
}

/// File types
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u8)]
#[non_exhaustive]
pub enum FileType {
    NamedPipe = 1,
    Directory = 4,
    Regular = 8,
    Link = 10,
}

/// A directory entry 
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub file_type: FileType,
    pub name: String,
}


/// Description: internally used for `readdir` syscall for passing data between kernel and user space
// 256 name length limit, see: https://man7.org/linux/man-pages/man3/readdir.3.html
#[derive(Debug)]
#[repr(C)]
pub struct RawDirent {
    pub d_type: usize,     // type of file
    pub d_name: [u8; 256], // null terminated entry name
}

impl RawDirent {
    pub fn new() -> Self {
        RawDirent {
            d_type: 0,
            d_name: [0; 256],
        }
    }

    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self as *mut RawDirent as *mut u8
    }
}

