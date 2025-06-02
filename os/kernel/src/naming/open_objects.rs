/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: open_objects                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Managing opened objects in a global table (OPEN_OBJECTS). And providing ║
   ║ all major functions for the naming service.                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 30.12.2024               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::string::String;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::result::Result;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::{Mutex, Once};

use super::traits::NamedObject;
use super::lookup;
use naming::shared_types::{DirEntry, OpenOptions, SeekOrigin};
use syscall::return_vals::{Errno, SyscallResult};


/// Max. number of open objetcs
const MAX_OPEN_OBJECTS: usize = 0x1000;

/// Helper function returning safe access to the open object table (oot)
fn get_open_object_table() -> Arc<Mutex<Box<OpenObjectTable>>> {
    OPEN_OBJECTS.get().unwrap().clone()
}

static OPEN_OBJECTS: Once<Arc<Mutex<Box<OpenObjectTable>>>> = Once::new();

struct OpenObjectTable {
    open_handles: Vec<(usize, Option<Arc<OpenedObject>>)>,
    free_handles: Box<[usize; MAX_OPEN_OBJECTS]>,
}


pub(super) fn open_object_table_init() {
    OPEN_OBJECTS.call_once(|| Arc::new(Mutex::new(OpenObjectTable::new())));
}

pub(super) fn open(path: &String, flags: OpenOptions) -> Result<usize, Errno> {
    // try to open the named object for the given path
    let result = lookup::lookup_named_object(path);
    if result.is_err() {
        return Err(Errno::ENOENT);
    }
    let found_named_object: NamedObject = result.unwrap();

    // check if path is a directory and this was requested
    if flags.contains(OpenOptions::DIRECTORY) {
        if !found_named_object.is_dir() {
            return Err(Errno::ENOTDIR);
        }
    }

    // try to allocate an new handle
    get_open_object_table()
        .lock()
        .allocate_handle(Arc::new(OpenedObject::new(
            Arc::new(found_named_object),
            AtomicUsize::new(0),
            flags,
        )))
}

pub(super) fn write(fh: usize, buf: &[u8]) -> Result<usize, Errno> {
    get_open_object_table()
        .lock()
        .lookup_opened_object(fh)
        .and_then(|opened_object| {
            // Make `opened_object` mutable here
            opened_object.named_object.as_file().and_then(|file| {
                let pos = opened_object.pos.load(Ordering::SeqCst);
                let bytes_written = file.write(buf, pos, opened_object.options)?;
                opened_object
                    .pos
                    .store(pos + bytes_written, Ordering::SeqCst);
                Ok(bytes_written) // Return the bytes written
            })
        })
}

pub(super) fn read(fh: usize, buf: &mut [u8]) -> Result<usize, Errno> {
    get_open_object_table()
        .lock()
        .lookup_opened_object(fh)
        .and_then(|opened_object| {
            // Make `opened_object` mutable here
            opened_object.named_object.as_file().and_then(|file| {
                let pos = opened_object.pos.load(Ordering::SeqCst);
                let bytes_read = file.read(buf, pos, opened_object.options)?;
                opened_object.pos.store(pos + bytes_read, Ordering::SeqCst);
                Ok(bytes_read) // Return the bytes read
            })
        })
}

pub fn seek(fh: usize, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
    get_open_object_table()
        .lock()
        .lookup_opened_object(fh)
        .and_then(|opened_object| {
            // Make `opened_object` mutable here
            opened_object.named_object.as_file().and_then(|file| {
                let new_pos = match origin {
                    SeekOrigin::Start => offset,
                    SeekOrigin::End => file.stat()?.size + offset,
                    SeekOrigin::Current => opened_object.pos.load(Ordering::SeqCst) + offset,
                };
                opened_object.pos.store(new_pos, Ordering::SeqCst);
                Ok(0) // Success
            })
        })
}

pub(super) fn readdir(fh: usize) -> Result<Option<DirEntry>, Errno> {
    get_open_object_table()
        .lock()
        .lookup_opened_object(fh)
        .and_then(|opened_object| {
            // Make `opened_object` mutable here
            opened_object.named_object.as_dir().and_then(|dir| {
                let pos = opened_object.pos.load(Ordering::SeqCst);
                let dir_entry = dir.readdir(pos)?;
                opened_object.pos.store(pos + 1, Ordering::SeqCst);
                Ok(dir_entry) // Return the DirEntry
            })
        })
}

pub(super) fn close(handle: usize) -> Result<usize, Errno> {
    get_open_object_table().lock().free_handle(handle)
}

/*pub(super) fn dump() {
    get_open_object_table().lock().dump();
}*/


impl OpenObjectTable {
    /// Create a new OpenObjectTable
    fn new() -> Box<OpenObjectTable> {
        Box::new(OpenObjectTable {
            open_handles: Vec::new(),
            free_handles: Box::new([0; MAX_OPEN_OBJECTS]),
        })
    }

    /// Lookup an 'OpenedObject' for a given handle
    fn lookup_opened_object(
        &mut self,
        opened_object_handle: usize,
    ) -> Result<&mut Arc<OpenedObject>, Errno> {
        self.open_handles
            .iter_mut()
            .find(|(h, _)| *h == opened_object_handle)
            .ok_or(Errno::EINVALH) // Handle not found in the table
            .and_then(|(_, named_obj)| named_obj.as_mut().ok_or(Errno::EINVALH))
    }

    /// Allocate a new handle for a given 'OpenObject'
    fn allocate_handle(&mut self, opened_object: Arc<OpenedObject>) -> Result<usize, Errno> {
        let res = self.find_free_handle();
        if res.is_none() {
            return Err(Errno::ENOHANDLES);
        }
        let new_handle = res.unwrap();
        self.open_handles.push((new_handle, Some(opened_object)));
        Ok(new_handle)
    }

    /// Free handle
    fn free_handle(&mut self, opened_object_handle: usize) -> SyscallResult {
        // Find the position of the file handle in the `open_handles` vector
        if let Some(index) = self
            .open_handles
            .iter()
            .position(|(h, _)| *h == opened_object_handle)
        {
            // Remove the handle from `open_handles`
            self.open_handles.swap_remove(index);
            // set handle as free
            self.free_handles[index] = 0;
            Ok(0)
        } else {
            // Handle not found
            Err(Errno::EINVALH)
        }
    }

    /// Helper function of 'allocate' to find a free handle
    fn find_free_handle(&mut self) -> Option<usize> {
        self.free_handles
            .iter_mut()
            .position(|value| *value == 0)
            .map(|index| {
                self.free_handles[index] = 1;
                index
            })
    }

/*
    fn dump(&self) {
        info!("OpenObjectTable: dumping used handles");
        for (handle, opened_object) in &self.open_handles {
            if let Some(opened_object) = opened_object {
                info!(
                    "    handle = {:?}, named object = {:?}",
                    handle, opened_object.named_object
                );
            }
        }
    }
    */
}

// Opened object stored in the 'OpenObjectTable'
// (includes NamedObject, current position within object, and options)
pub struct OpenedObject {
    named_object: Arc<NamedObject>,
    pos: AtomicUsize, // current position within file or number of next DirEntry
    options: OpenOptions,
}

impl OpenedObject {
    pub fn new(named_object: Arc<NamedObject>, pos: AtomicUsize, options: OpenOptions) -> OpenedObject {
        OpenedObject {
            named_object,
            pos,
            options,
        }
    }
}
