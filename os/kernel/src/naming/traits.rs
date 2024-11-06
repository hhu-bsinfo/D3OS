/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: traits                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All internal traits used within the naming service (ns).        ║
   ║         - NsInterface: specifies all operations provided by the ns      ║
   ║         - NsNode: trait defining all operations on a named object       ║
   ║         - NsNodeDirectory: specifies all operations on a directory      ║
   ║         - NsNodeFile: trait defining all operations on a file           ║
   ║         - NsOpenFile: specifies all operations on an open file          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 12.9.2024                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::string::String;
use core::fmt::Debug;
use core::result::Result;

use syscall::return_vals::{OpenOptions, Errno, SeekOrigin};


/// Types of a node stored in the naming service
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NsNodeType {
    File,
    Directory,
}

/// `NsInterface` specifies all operations provided by the naming service
pub(super) trait NsInterface: Debug + Send + Sync {
    /// Create a directory (including all sub directories)
    fn mkdir(&self, path: &String) -> Result<(),Errno>;
    
    /// Open the file given in `path` (must be absolute)
    /// Options for opening files
    /// Returns a file handle on success
    fn open(&self, path: &String, flags: OpenOptions) -> Result<Box<dyn NsOpenFile>, Errno>;

    /// Dump all nodes in the naming service (for debugging)
    fn dump(&self);
}

/// `NsNode` defines all operations for a node in the the ns
pub(super) trait NsNode: Debug + Send + Sync {
    /// Determines the current node type
    fn get_type(&self) -> NsNodeType;
}

/// `NsNodeFile` represents a file node of the naming service
pub trait NsNodeFile: NsNode + Debug + Send + Sync {
	/// Create a file handle to the current file
	fn get_handle(&self, _opt: OpenOptions) -> Result<Box<dyn NsOpenFile>, Errno>;
}


/// `NsNodeDirectory` specifies all operations on a directory
pub(super) trait NsNodeDirectory : NsNode + Debug + Send + Sync
{
    /// Helper function to create a new dirctory node
    fn mkdir(&self, _components: &mut Vec<&str>) -> Result<(),Errno>;

    /// Helper function to open a file
    fn open(
        &self,
        path: &mut Vec<&str>,
        _flags: OpenOptions,
    ) -> Result<Box<dyn NsOpenFile>, Errno>;

    /// Helper function to print the current state of the file system
    fn dump(&self, _tabs: String);
}

/// Description: This trait defines all functions that can be applied to an open file
#[allow(dead_code)] // size() is currently not used and produces a compiler warning. May be removed late, once all methods are in active use.
pub(super) trait NsOpenFile: Debug + Send + Sync {

    ///
    /// Description: \
    ///    Read bytes from the file (from current position) into the given buffer. \
    ///    The number of bytes to be read is determined by the buffer size 
    ///
    /// Parameters: `buf` buffer to copy file bytes into
    ///
    /// Return: `Ok(#bytes read)` or `Err(errno)`
    ///
    fn read(&self, buf: &mut [u8]) -> Result<usize, Errno>;

    ///
    /// Description: \
    ///    Write bytes from the given buffer into the file (at the current position). \
    ///    The number of bytes to be written is determined by the buffer size 
    ///
    /// Parameters: `buf` buffer from which bytes are copied into the file
    ///
    /// Return: `Ok(#bytes written)` or `Err(errno)`
    ///
    fn write(&self, buf: &[u8]) -> Result<usize, Errno>;

    ///
    /// Description: Get file size.
    ///
    /// Return: `Ok(size in bytes)` or `Err(errno)`
    ///
    fn size(&self) -> usize;

    ///
    /// Description: Set file pointer.
    ///
    /// Parameters: \
    ///    `offset` offset in bytes \
    ///    `origin` point of origin
    ///
    /// Return: `Ok( in bytes)` or `Err(errno)`
    ///
    fn seek(&self, offset: usize, origin: SeekOrigin) -> Result<usize, Errno>;
}