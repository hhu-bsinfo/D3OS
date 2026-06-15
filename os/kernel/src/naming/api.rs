/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: api                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Public functions of the naming service:                                 ║
   ║   - init   init ns, called once                                         ║
   ║   - open   open a named object                                          ║
   ║   - read   read bytes from an open object                               ║
   ║   - write  write bytes into an open object                              ║
   ║   - seek   set file pointer (for files)                                 ║
   ║   - mkdir  create a directory                                           ║
   ║   - touch  create a file                                                ║
   ║   - mkfifo create a named pipe                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 25.8.2025                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::format;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Pointer};
use core::ptr::read_unaligned;
use core::sync::atomic::Ordering;
use log::{error, info, warn};
use spin::{Mutex, Once};

use super::lookup;
use super::open_objects;
use super::stat::Mode;
use super::tmpfs;
use super::traits::{FileSystem, NamedObject};

use crate::initrd;
use naming::shared_types::{OpenOptions, RawDirent, SeekOrigin};
use syscall::return_vals::Errno;
use crate::capabilities::capability::{Capability, CapabilityFlags};
use crate::capabilities::capability_objects::naming_object::{create_naming_capability, NamingObject, ObjectType};
use crate::syscall::sys_vmem::init_fb_info;

// root of naming service
pub(crate) static ROOT: Once<Arc<dyn FileSystem>> = Once::new();

// current working directory
static CWD: Mutex<String> = Mutex::new(String::new());

/// Initialize the naming service (must be called once before using it).
pub fn init() {
    // Initialize ROOT with TmpFs
    ROOT.call_once(|| {
        let tmpfs = tmpfs::TmpFs::new();

        for entry in initrd().entries() {
            let res = tmpfs.create_static_file(entry.filename().as_str().unwrap(), entry.data());
            if res.is_err() {
                warn!("Failed to create static file in tmpfs: {}", entry.filename().as_str().unwrap());
            }
        }

        Arc::new(tmpfs)
    });
    //open_objects::open_object_table_init();
    let mut cwd = CWD.lock();
    *cwd = "/".to_string();
    info!("naming service initialized");
    //    test::running_tests();
}

/*pub(crate) fn root() -> Result<Capability<NamingObject>, Errno> { //Every threat can access Root dir
    match open_objects::open("/", OpenOptions::all()){
        Ok(root) => {
            Ok(create_naming_capability(root, OpenOptions::all(), None))
        },
        Err(e) => {
            error!("root not found");
            Err(e)
        },
    }
}*/

pub(crate) fn shared_pipe(cap_to_dir: &Capability<NamingObject>) -> Capability<NamingObject> {
    open_shared_pipe("shared_pipe", OpenOptions::READWRITE | OpenOptions::CREATE, cap_to_dir).unwrap_or_else(|_| {
        error!("could not create shared pipe");
        Capability::null()
    })
}

pub(crate) fn root() -> Capability<NamingObject> { //Every threat can access Root dir
    match open_objects::open("/", OpenOptions::READWRITE | OpenOptions::SHARE){
        Ok(root) => {
            create_naming_capability(root, OpenOptions::all(), "/".to_string())
        },
        Err(e) => {
            error!("root not found, error: {:?}", e);
            Capability::null()
        },
    }
}

/// Open/create a named object referenced by `path` using the given `flags`. \
/// Returns `Ok(object_handle)` or `Err`.
pub fn open(flags: OpenOptions, file_cap: &Capability<NamingObject>) -> Result<Capability<NamingObject>, Errno> {
    // Verify the original capability has required permissions
    if !file_cap.has_permissions(CapabilityFlags::READ | CapabilityFlags::WRITE) {
        error!("no permssion to open object with given capability");
        return Err(Errno::EACCES);
    }

    // Get the underlying object
    let Some(naming_obj) = file_cap.invoke() else {
        error!("could not invoke capability");
        return Err(Errno::EINVAL);
    };

    // Handle pipes differently from files
    match open_objects::open(&naming_obj.path, flags) {
        Ok(obj) => {
            // info!("opened object at path: {}, returning OK", &naming_obj.path);
            Ok(create_naming_capability(obj, flags, naming_obj.path.to_string()))
        },
        Err(e) => Err(e)
    }
}



// pub fn open(path: &str, flags: OpenOptions, cap_to_dir: &Capability<NamingObject>) -> Result<Capability<NamingObject>, Errno> {
// //avoid "opening" a file twice (only the creator receives                                                                 capability and then needs to share it)
//     let result = lookup::lookup_named_object(path);
//     if result.is_ok() {
//         return Err(Errno::EEXIST);
//     }
//
//     // Try to open the object
//     open_object(path, flags, cap_to_dir)
// }≥

/// Write all bytes from the given `buffer` into the named object referenced by `object_handle`. \
/// Returns `Ok(number of bytes written)` or `Err`.
/*
pub fn write(object_handle: usize, buffer: &[u8]) -> Result<usize, Errno> {
    open_objects::write(object_handle, buffer)
}
 */

pub fn write(cap: &Capability<NamingObject>, buffer: &[u8]) -> Result<usize, Errno> {
    if let Some(naming_obj) = cap.invoke() {
        if naming_obj.named_object.is_file() {
            // Make `opened_object` mutable here
            return naming_obj.named_object.as_file().and_then(|file| {
                let pos = naming_obj.position.load(Ordering::SeqCst);
                let bytes_written = file.write(buffer, pos, naming_obj.access_rights)?;
                naming_obj.position.store(pos + bytes_written, Ordering::SeqCst);
                Ok(bytes_written) // Return the bytes written
            });
        }
        if naming_obj.named_object.is_pipe() {
            // Make `opened_object` mutable here
            return naming_obj.named_object.as_pipe().and_then(|pipe| {
                let bytes_written = pipe.write(buffer, 0, naming_obj.access_rights)?;
                // info!("pipe written: {:?}, {} byte(s)", buffer, bytes_written);
                Ok(bytes_written) // Return the bytes written
            });
        }
        Err(Errno::ENOTSUP)
    } else {
        // info!("could not invoke capability");
        Err(Errno::EACCES)
    }
}

/// Read from the named object referenced by `object_handle` into the given `buffer`. \
/// Returns `Ok(number of bytes read)` or `Err`.
/*
pub fn read(object_handle: usize, buffer: &mut [u8]) -> Result<usize, Errno> {
    open_objects::read(object_handle, buffer)
}
 */

pub fn read(cap: &Capability<NamingObject>, buffer: &mut [u8]) -> Result<usize, Errno> {
    if let Some(naming_obj) = cap.invoke() {
        if naming_obj.named_object.is_file() {
            // Make `opened_object` mutable here
            return naming_obj.named_object.as_file().and_then(|file| {
                let pos = naming_obj.position.load(Ordering::SeqCst);
                let bytes_read = file.read(buffer, pos, naming_obj.access_rights)?;
                naming_obj.position.store(pos + bytes_read, Ordering::SeqCst);
                Ok(bytes_read) // Return the bytes read
            });
        }
        if naming_obj.named_object.is_pipe() {
            // Make `opened_object` mutable here
            return naming_obj.named_object.as_pipe().and_then(|pipe| {
                let bytes_read = pipe.read(buffer, 0, naming_obj.access_rights)?;
                Ok(bytes_read) // Return the bytes written
            });
        }
        Err(Errno::ENOTSUP)

    } else {
        Err(Errno::EACCES)
    }
}

/// Move the object pointer for the named object referenced by `object_handle` to the specified `offset` from the `origin`. \
/// Returns `Ok(nr of bytes seeked)` or `Err`.
pub fn seek(cap: &Capability<NamingObject>, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
    if let Some(naming_obj) = cap.invoke() {
        if naming_obj.named_object.is_file() {
            // Make `opened_object` mutable here
            return naming_obj.named_object.as_file().and_then(|file| {
                let new_pos = match origin {
                    SeekOrigin::Start => offset,
                    SeekOrigin::End => file.stat()?.size + offset,
                    SeekOrigin::Current => naming_obj.position.load(Ordering::SeqCst) + offset,
                };
                naming_obj.position.store(new_pos, Ordering::SeqCst);
                Ok(new_pos) // Success
            });
        }
        Err(Errno::ENOTSUP)
    } else {
        Err(Errno::EACCES)
    }
}

/// Close the named object referenced by `object_handle`.
/// Returns `Ok(0)` or `Err(errno)`
/*pub fn close(object_handle: usize) -> Result<usize, Errno> {
    open_objects::close(object_handle)
}
 */

pub fn close(cap: &Capability<NamingObject>) -> Result<usize, Errno> {
    if let Some(naming_obj) = cap.invoke() {
        if naming_obj.named_object.is_file() || naming_obj.named_object.is_pipe() {
            // Dropping the capability will close the object if this is the last reference
            Ok(0) // Success
        } else {
            Err(Errno::ENOTSUP)
        }
    } else {
        Err(Errno::EACCES)
    }
}
/// Create a directory named 'name' in the directory given by the capability object. \
/// Returns `Ok(Capability<NamingObject>)` or `Err(errno)`
pub fn mkdir(name: &str, flags: OpenOptions, parent_dir: &Capability<NamingObject>) -> Result<Capability<NamingObject>, Errno> {
    // Check rights of parent cap
    if let Some(dir)  = parent_dir.invoke(){
        if dir.access_rights.intersects(OpenOptions::CREATE) {
            return dir.named_object.as_dir().and_then(|directory| {
                if let Ok(obj) = directory.create_dir(name, Mode::new(0)) {
                    Ok(create_naming_capability(obj, flags, dir.path.to_string() + name)) // Successfully created the directory
                } else { Err(Errno::EACCES) }
            });
        }
    }
    Err(Errno::EACCES)
    
    /*
    // Split the path into components
    let mut components: Vec<&str> = path.split("/").collect();

    // Remove the last component (the name of the new directory)
    let new_dir_name = components.pop();

    // We need parent directory to create the new directory
    let parent_dir = if components.len() == 1 {
        "/".to_string()
    } else {
        components.join("/") // Joins the remaining components
    };

    // Safely lookup the parent directory and create the new file
    let result = lookup::lookup_dir(&parent_dir)
        .and_then(|dir| {
            new_dir_name
                .ok_or(Errno::EINVAL) // Handle missing file name
                .and_then(|name| dir.create_dir(name, Mode::new(0))) // Create the file
        });
        
     */
}

/// Create an empty file defined by `path`. \
/// Returns `Ok(0)` or `Err(errno)`
pub fn touch(name: &str, flags: OpenOptions, dir_cap: &Capability<NamingObject>) -> Result<Capability<NamingObject>, Errno> {
    // Verify we have a valid filename
    if name.is_empty() {
        return Err(Errno::EINVAL);
    }

    // Ensure we don't try to process paths with '/' in them since we already have the directory capability
    if name.contains('/') {
        return Err(Errno::EINVAL);
    }

    // Safely lookup the parent directory and create the new file
    let Some(naming_obj) = dir_cap.invoke() else {
        error!("touch: could not invoke capability");
        return Err(Errno::EACCES);
    };


    if naming_obj.named_object.is_dir() && naming_obj.access_rights.contains(OpenOptions::READWRITE) {
        let result = naming_obj.named_object
            .as_dir()
            .and_then(|dir| dir.create_file(name, Mode::new(0))); // Create the file)


        return match result {
            Ok(obj) => Ok(create_naming_capability(obj, flags, naming_obj.path.to_string() + name)), // Successfully created the file
            Err(_) => {
                // Handle the error here (e.g., logging or returning the error code)
                error!("touch: could not create file: {}", name);
                Err(Errno::ENOTDIR)
            }
        }
    }
    Err(Errno::EINVAL)
}

/// Read next directory entry of directory referenced by `dir_handle` \
/// Returns: \
///   `Ok(1)` next directory entry in `dentry` \
///   `Ok(0)` no more entries in the directory \
///   `Err`   error code
pub fn readdir(dir_handle: usize, dentry: Option<&mut RawDirent>) -> Result<usize, Errno> {
    return Err(Errno::ENOTSUP);
    let res = open_objects::readdir(dir_handle);
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
                    if let Some(dentry) = dentry {
                        *dentry = de;
                        Ok(1) // Indicate success
                    } else {
                        Err(Errno::EUNKN) // Handle null pointer case
                    }
                }
                None => Ok(0),
            }
        }
        Err(e) => Err(e),
    }
}

/// Get the current working directory and return path in `buffer`. \
/// Return: `Ok(len of string)` or `Err(errno)`
pub fn cwd(buffer: &mut [u8]) -> Result<usize, Errno> {
    return Err(Errno::ENOTSUP);
    let cwd = CWD.lock();

    // Get the string as bytes
    let cwd_bytes = cwd.as_bytes();

    // Calculate how much data can be copied (leave space for the null terminator)
    let len_to_copy = (buffer.len() - 1).min(cwd_bytes.len()); // Reserve space for the null terminator

    // Copy the data into the buffer
    buffer[..len_to_copy].copy_from_slice(&cwd_bytes[..len_to_copy]);

    // Add the null terminator if there is space
    if buffer.len() > len_to_copy {
        buffer[len_to_copy] = 0;
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
    return Err(Errno::ENOTSUP);
    let result = lookup::lookup_dir(path);
    match result {
        Ok(_) => {
            let mut cwd = CWD.lock();
            *cwd = path.clone();
            Ok(0)
        }
        Err(_) => {
            // Handle the error here (e.g., logging or returning the error code)
            Err(Errno::ENOTDIR)
        }
    }
}


pub fn mkfifo(name: &str, flags: OpenOptions, dir_cap: &Capability<NamingObject>) -> Result<Capability<NamingObject>, Errno> {
    // Verify we have a valid filename
    if name.is_empty() {
        return Err(Errno::EINVAL);
    }

    // Ensure we don't try to process paths with '/' in them since we already have the directory capability
    if name.contains('/') {
        return Err(Errno::EINVAL);
    }

    // Get the directory from the capability
    let Some(dir_obj) = dir_cap.invoke() else {
        error!("mkfifo: could not invoke directory capability");
        return Err(Errno::EACCES);
    };

    // Verify we have a directory with create permissions
    if !dir_obj.named_object.is_dir() || !dir_obj.access_rights.contains(OpenOptions::CREATE) {
        error!("mkfifo: no permission to create in directory");
        return Err(Errno::EACCES);
    }

    // Create the pipe in the directory
    let result = dir_obj.named_object
        .as_dir()
        .and_then(|dir| dir.create_pipe(name, Mode::new(0)));

    match result {
        Ok(pipe_obj) => {
            // info!("mkfifo: created pipe '{}'", name);
            // Create capability with full permissions since this is the original capability
            Ok(create_naming_capability(
                pipe_obj,
                flags | OpenOptions::SHARE,  // Include SHARE permission for the original capability
                format!("{}/{}", dir_obj.path.trim_end_matches('/'), name)
            ))
        },
        Err(e) => {
            error!("mkfifo: failed to create pipe '{}': {:?}", name, e);
            Err(Errno::EACCES) //hide error details to avoid sec leak
        }
    }
}


fn open_shared_pipe(name: &str, flags: OpenOptions, capability_to_dir: &Capability<NamingObject>) -> Result<Capability<NamingObject>, Errno> { //only used for shared pipe (temporary
    let path = if let Some(parent_dir) = capability_to_dir.invoke() {
        &*(parent_dir.path.clone() + name)
    } else {
        return Err(Errno::EACCES);
    };

    match open_objects::open(path, flags){
        Ok(obj) => {
            info!("opened object at path: {}", name);
            Ok(create_naming_capability(obj, flags, "/".to_string() + name))
        },
        Err(e) => {
            if flags.contains(OpenOptions::CREATE) && e != Errno::EEXIST {
                warn!("could not open object at path: {}, error: {:?}. Trying to create it.", path, e);
                mkfifo(name, flags, capability_to_dir) //Problem here
            } else {
                Err(e)
            }
        },
    }
}