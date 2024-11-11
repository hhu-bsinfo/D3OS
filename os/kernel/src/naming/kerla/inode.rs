use alloc::sync::Arc;
use syscall::return_vals::Errno;
use core::fmt::Debug;

use super::opened_file::{Downcastable, OpenOptions};


/// A file-like object.
///
/// This trait represents an object which behaves like a file such as files on
/// disks (aka. regular files), UDP/TCP sockets, device files like tty, etc.
pub trait FileLike: Debug + Send + Sync + Downcastable {
    /// `open(2)`.
    fn open(&self, _options: &OpenOptions) -> Result<Option<Arc<dyn FileLike>>, Errno> {
        Ok(None)
    }
}

/// Represents a directory.
pub trait Directory: Debug + Send + Sync + Downcastable {
    /// Looks for an existing file.
    fn lookup(&self, name: &str) -> Result<INode, Errno>;
}

#[derive(Clone)]
pub enum INode {
    FileLike(Arc<dyn FileLike>),
    Directory(Arc<dyn Directory>),
}

impl INode {
    /// Unwraps as a file. If it's not, returns `Errno::EBADF`.
    pub fn as_file(&self) -> Result<&Arc<dyn FileLike>, Errno> {
        match self {
            INode::FileLike(file) => Ok(file),
            _ => Err(Errno::EUNKN),
        }
    }

    /// Unwraps as a directory. If it's not, returns `Errno::EBADF`.
    pub fn as_dir(&self) -> Result<&Arc<dyn Directory>, Errno> {
        match self {
            INode::Directory(dir) => Ok(dir),
            _ => Err(Errno::EUNKN),
        }
    }

    /// Returns `true` if it's a file.
    pub fn is_file(&self) -> bool {
        matches!(self, INode::FileLike(_))
    }

    /// Returns `true` if it's a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self, INode::Directory(_))
    }
}

impl From<Arc<dyn FileLike>> for INode {
    fn from(file: Arc<dyn FileLike>) -> Self {
        INode::FileLike(file)
    }
}
