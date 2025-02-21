/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: api                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Public interface of the naming service:                         ║
   ║ Funcs:                                                                  ║
   ║   - init:  init ns, called once                                         ║
   ║   - open:  open a named object                                          ║
   ║   - read:  read bytes from an open object                               ║
   ║   - write: write bytes into an open object                              ║
   ║   - seek:  set file pointer (for files)                                 ║
   ║   - mkdir: create a directory                                           ║
   ║   - touch: create a file                                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 30.12.2024               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::info;
use spin::{Mutex, Once};

use super::traits::FileSystem;
use super::lookup;
use super::open_objects;
use super::stat::Mode;
use super::tmpfs;

use naming::shared_types::{OpenOptions, RawDirent, SeekOrigin};
use syscall::return_vals::Errno;

// root of naming service
pub(super) static ROOT: Once<Arc<dyn FileSystem>> = Once::new();

// current working directory
static CWD: Mutex<String> = Mutex::new(String::new());

///
/// Description: Initilize the naming service (must be called once before using it)
///
pub fn init() {
    // Initialize ROOT with TmpFs
    ROOT.call_once(|| {
        let tmpfs = tmpfs::TmpFs::new();
        Arc::new(tmpfs)
    });
    open_objects::open_object_table_init();
    let mut cwd = CWD.lock();
    *cwd = "/".to_string();
    info!("naming service initialized");
    //    test::running_tests();
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
pub fn open(path: &String, flags: OpenOptions) -> Result<usize, Errno> {
    open_objects::open(path, flags).or_else(|e| {
        if flags.contains(OpenOptions::CREATE) {
            touch(path).and_then(|_| open_objects::open(path, flags))
        } else {
            Err(e)
        }
    })
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
pub fn write(fh: usize, buf: &[u8]) -> Result<usize, Errno> {
    open_objects::write(fh, &buf)
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
pub fn read(fh: usize, buf: &mut [u8]) -> Result<usize, Errno> {
    open_objects::read(fh, buf)
}

///
/// Description: Set file pointer.
///
/// Parameters: \
///    `fh`  file handle \
///    `offset` offset in bytes \
///    `origin` point of origin
///
/// Return: `Ok(0)` or `Err(errno)`
///
pub fn seek(fh: usize, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
    open_objects::seek(fh, offset, origin)
}

///
/// Description: Close an open object \
/// Return: `Ok(0)` or `Err(errno)`
///
pub fn close(fh: usize) -> Result<usize, Errno> {
    open_objects::close(fh)
}

///
/// Description: Create a directory for the given path \
/// Parameters: `path` absolute path \
/// Return: `Ok(0)` or `Err(errno)`
///
pub fn mkdir(path: &String) -> Result<usize, Errno> {
    // Split the path into components
    let mut components: Vec<&str> = path.split("/").collect();

    // Remove the last component (the name of the new directory)
    let new_dir_name = components.pop();

    // We need parent directory to create the new directory
    let parent_dir;
    if components.len() == 1 {
        parent_dir = "/".to_string();
    } else {
        parent_dir = components.join("/"); // Joins the remaining components
    }

    // Safely lookup the parent directory and create the new file
    let result = lookup::lookup_dir(&parent_dir)
        .and_then(|dir| {
            new_dir_name
                .ok_or(Errno::EINVAL) // Handle missing file name
                .and_then(|name| dir.create_dir(name, Mode::new(0))) // Create the file
        })
        .map(|_| 0); // Convert the success result to 0

    match result {
        Ok(_) => Ok(0), // Successfully created the file
        Err(_) => {
            // Handle the error here (e.g., logging or returning the error code)
            Err(Errno::ENOTDIR)
        }
    }
}

///
/// Description: Create a new empty file \
/// Parameters: `path` absolute path \
/// Return: `0` if successful, otherwise an error code
///
pub fn touch(path: &String) -> Result<usize, Errno> {
    // Split the path into components
    let mut components: Vec<&str> = path.split("/").collect();

    // Remove the last component (the name of the new file)
    let new_file_name = components.pop();

    // We need parent directory to create the new file
    let parent_dir;
    if components.len() == 1 {
        parent_dir = "/".to_string();
    } else {
        parent_dir = components.join("/"); // Joins the remaining components
    }

    // Safely lookup the parent directory and create the new file
    let result = lookup::lookup_dir(&parent_dir)
        .and_then(|dir| {
            new_file_name
                .ok_or(Errno::EINVAL) // Handle missing file name
                .and_then(|name| dir.create_file(name, Mode::new(0))) // Create the file
        })
        .map(|_| 0); // Convert the success result to 0

    match result {
        Ok(_) => Ok(0), // Successfully created the file
        Err(_) => {
            // Handle the error here (e.g., logging or returning the error code)
            Err(Errno::ENOTDIR)
        }
    }
}

///
/// Description: \
///    Read next directory entry
///
/// Parameters: \
///    `fh`     handle to an open directory \
///    `dentry` memory for the next directory entry to be returned \
///
/// Return: \
///    `Ok(1)` next directory entry in `dentry` \
///    `Ok(0)` no more entries in the directory \
///    `Err`    error code
///
pub fn readdir(fh: usize, dentry: *mut RawDirent) -> Result<usize, Errno> {
    let res = open_objects::readdir(fh);
    match res {
        Ok(dir_entry) => {
            match dir_entry {
                Some(dir_entry_data) => {
                    // copy data
                    let mut de: RawDirent = RawDirent::new();
                    de.d_type = dir_entry_data.file_type as usize;
                    let name_bytes: &[u8] = dir_entry_data.name.as_bytes();
                    let len = name_bytes.len().min(255); // Avoid overflow
                    de.d_name[..len].copy_from_slice(&name_bytes[..len]);

                    // Write the Dirent structure to the provided dentry pointer
                    unsafe {
                        if !dentry.is_null() {
                            *dentry = de;
                            return Ok(1); // Indicate success
                        } else {
                            return Err(Errno::EUNKN); // Handle null pointer case
                        }
                    }
                }
                None => Ok(0),
            }
        }
        Err(e) => Err(e),
    }
}

///
/// Description: \
///    Return the current working path.
///
/// Parameters: \
///    `buf` buffer to copy current path into \
///
/// Return: `Ok(#len of string)` or `Err(errno)`
///
pub fn cwd(buf: &mut [u8]) -> Result<usize, Errno> {
    // Lock the CWD mutex to access its value
    let cwd = CWD.lock();
    
    // Get the string as bytes
    let cwd_bytes = cwd.as_bytes();

    // Calculate how much data can be copied (leave space for the null terminator)
    let len_to_copy = (buf.len() - 1).min(cwd_bytes.len()); // Reserve space for the null terminator

    // Copy the data into the buffer
    buf[..len_to_copy].copy_from_slice(&cwd_bytes[..len_to_copy]);

    // Add the null terminator if there is space
    if buf.len() > len_to_copy {
        buf[len_to_copy] = 0;
    }

    // Return the total length including the null terminator, or just the copied length
    Ok(len_to_copy + 1)
}

///
/// Description: Change working directory \
/// Parameters: `path` absolute path \
/// Return: `Ok(0)` or `Err(errno)`
///
pub fn cd(path: &String) -> Result<usize, Errno> {
    let result = lookup::lookup_dir(&path);
    match result {
        Ok(_) => {
            let mut cwd = CWD.lock();
            *cwd = path.clone();
            Ok(0)
        },
        Err(_) => {
            // Handle the error here (e.g., logging or returning the error code)
            Err(Errno::ENOTDIR)
        }
    }
}
