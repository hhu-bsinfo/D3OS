/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: traits                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All internal traits used within the naming service (ns).        ║
   ║         - FileSystem: type and operations for a file system             ║
   ║         - NamedObject: generic type of an object                        ║
   ║         - DirectoryObject: specifies all operations on a directory obj. ║
   ║         - FileObject: specifies all operations on a file object         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 30.12.2024               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/


use alloc::sync::Arc;
use core::fmt::{self, Debug};
use core::result::Result;

use super::stat::{Mode, Stat};
use naming::shared_types::{OpenOptions, DirEntry};
use syscall::return_vals::Errno;

/// FileSystem operations
pub trait FileSystem: Send + Sync {
    fn root_dir(&self) -> Arc<dyn DirectoryObject>;
}

/// File object operations
pub trait FileObject: Debug + Send + Sync {
    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::EBADF)
    }

    fn read(&self, _buf: &mut [u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::EBADF)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::EBADF)
    }
}


/// Directory object operations
pub trait DirectoryObject: Debug + Send + Sync {
    fn lookup(&self, name: &str) -> Result<NamedObject, Errno>;
    fn create_file(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno>;
    fn create_dir(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno>;
    fn stat(&self) -> Result<Stat, Errno>;
    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno>;
}

/// A named object.
#[derive(Clone)]
pub enum NamedObject {
    FileObject(Arc<dyn FileObject>),
    DirectoryObject(Arc<dyn DirectoryObject>),
}

impl NamedObject {
    /// Unwraps as a file. If it's not, returns `Errno::EBADF`.
    pub fn as_file(&self) -> Result<&Arc<dyn FileObject>, Errno> {
        match self {
            NamedObject::FileObject(file) => Ok(file),
            _ => Err(Errno::EBADF),
        }
    }

    /// Unwraps as a directory. If it's not, returns `Errno::EBADF`.
    pub fn as_dir(&self) -> Result<&Arc<dyn DirectoryObject>, Errno> {
        match self {
            NamedObject::DirectoryObject(dir) => Ok(dir),
            _ => Err(Errno::EBADF),
        }
    }

    /// Returns `true` if it's a file.
    pub fn is_file(&self) -> bool {
        matches!(self, NamedObject::FileObject(_))
    }

    /// Returns `true` if it's a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self, NamedObject::DirectoryObject(_))
    }
}

impl fmt::Debug for NamedObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedObject::FileObject(file) => fmt::Debug::fmt(file, f),
            NamedObject::DirectoryObject(dir) => fmt::Debug::fmt(dir, f),
        }
    }
}

impl From<Arc<dyn FileObject>> for NamedObject {
    fn from(file: Arc<dyn FileObject>) -> Self {
        NamedObject::FileObject(file)
    }
}

impl From<Arc<dyn DirectoryObject>> for NamedObject {
    fn from(dir: Arc<dyn DirectoryObject>) -> Self {
        NamedObject::DirectoryObject(dir)
    }
}

pub fn as_named_object(dir: Arc<dyn DirectoryObject>) -> NamedObject {
    NamedObject::DirectoryObject(dir)
}

