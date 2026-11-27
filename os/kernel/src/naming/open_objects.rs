/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: open_objects                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Managing handles for opened objects per process as well as implementing ║
   ║ operations on opened objects through a handle.                          ║
   ║                                                                         ║
   ║ Public functions (called by api.rs):                                    ║
   ║   - open_object_table_init init the map of handle tables, called once   ║
   ║   - open                   open a named object, returns a handle        ║
   ║   - read                   read bytes through a handle                  ║
   ║   - write                  write bytes through a handle                 ║
   ║   - seek                   set file position (for files)                ║
   ║   - readdir                read next directory entry                    ║
   ║   - list_dir               list all entries of a directory              ║
   ║   - close                  close a handle                               ║
   ║   - delete                 delete the object and close its handle       ║
   ║   - close_all_for_process  close all handles of an exiting process      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 27.11.2026               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::result::Result;
use core::sync::atomic::{AtomicUsize, Ordering};
use spin::Once;
use spin::rwlock::RwLock;
use log::{info, debug, warn};

use crate::naming::traits::DirectoryObject;
use uuid::Uuid;

use super::lookup;
use super::traits::NamedObject;
use naming::shared_types::{DirEntry, OpenOptions, SeekOrigin};
use syscall::return_vals::Errno;

/// Max. number of open objects per process = 16384
const MAX_OPEN_OBJECTS: usize = 0x4000;

/// All open object tables of all processes
static OPEN_OBJECTS: Once<RwLock<BTreeMap<Uuid, Arc<OpenObjectTable>>>> = Once::new();

pub(super) fn open_object_table_init() {
    OPEN_OBJECTS.call_once(|| RwLock::new(BTreeMap::new()));
}

/// Helper function returning the map of all open object tables
fn tables() -> &'static RwLock<BTreeMap<Uuid, Arc<OpenObjectTable>>> {
    OPEN_OBJECTS.get().expect("open_objects: naming service not initialized")
}

/// Return the table of process `owner`. A process that has not opened anything
/// has no table, so all its handles are invalid (`EINVALH`).
fn table_of(owner: Uuid) -> Result<Arc<OpenObjectTable>, Errno> {
    let table = tables().read().get(&owner).cloned(); // map lock released here
    table.ok_or(Errno::EINVALH)
}

/// Return the table of process `owner`, creating it on its first open.
fn table_of_or_create(owner: Uuid) -> Arc<OpenObjectTable> {
    let existing = tables().read().get(&owner).cloned(); // map lock released here
    if let Some(table) = existing {
        return table;
    }
    // No table exists, create it
    tables()
        .write()
        .entry(owner)
        .or_insert_with(|| Arc::new(OpenObjectTable::new()))
        .clone()
}

/// Lookup the opened object for the given `handle` of process `owner`.
/// (Returns a clone of the `Arc`, so no lock is held afterwards)
fn lookup_opened_object(owner: Uuid, handle: usize) -> Result<Arc<OpenedObject>, Errno> {
    table_of(owner)?.lookup(handle)
}


/// Remove `handle` of process `owner` and return its opened object.
fn remove_opened_object(owner: Uuid, handle: usize) -> Result<Arc<OpenedObject>, Errno> {
    table_of(owner)?.remove(handle)
}

/// Try to open the named object for the given `path` for process `owner` 
pub(super) fn open(owner: Uuid, path: &str, flags: OpenOptions) -> Result<usize, Errno> {
    let result = lookup::lookup_named_object(path);
    if result.is_err() {
        return Err(Errno::ENOENT);
    }
    let (found_named_object, parent_dir) = result.unwrap();

    // check if path is a directory and this was requested
    if flags.contains(OpenOptions::DIRECTORY) {
        if !found_named_object.is_dir() {
            return Err(Errno::ENOTDIR);
        }
    }

    // call the 'open' for a directory with flag `WRITEONLY`
    if found_named_object.is_dir() {
        if flags.contains(OpenOptions::WRITEONLY) || flags.contains(OpenOptions::READWRITE) {
            return Err(Errno::EISDIR);
        }
    }

    // pipe rendezvous (may block), done before any table lock is taken
    let opened_pipe = if found_named_object.is_pipe() {
        let pipe = found_named_object.as_pipe()?.clone();
        pipe.open(flags)?; // ignore return value
        Some(pipe)
    } else {
        None
    };

    // get object name from path
    let object_name = lookup::get_filename_from_path(path).to_string();

    // try to allocate an new handle in the table of process `owner`
    let opened_object = Arc::new(OpenedObject::new(
        Arc::new(found_named_object),
        object_name,
        AtomicUsize::new(0),
        flags,
        parent_dir,
    ));
    let result = table_of_or_create(owner).allocate_handle(opened_object);

    if result.is_err() {
        if let Some(pipe) = opened_pipe {
            pipe.close(flags);
        }
    }

    result
}

/// `write` data in `buf` to object `handle` opened by process `owner`
pub(super) fn write(owner: Uuid, handle: usize, buf: &[u8]) -> Result<usize, Errno> {
    lookup_opened_object(owner, handle).and_then(|opened_object| {
        if opened_object.named_object.is_file() {
            return opened_object.named_object.as_file().and_then(|file| {
                let pos = opened_object.pos.load(Ordering::SeqCst);
                let bytes_written = file.write(buf, pos, opened_object.options)?;
                opened_object.pos.store(pos + bytes_written, Ordering::SeqCst);
                Ok(bytes_written) // Return the bytes written
            });
        }
        if opened_object.named_object.is_pipe() {
            return opened_object.named_object.as_pipe().and_then(|pipe| {
                let bytes_written = pipe.write(buf, 0, opened_object.options)?;
                Ok(bytes_written) // Return the bytes written
            });
        }

        if opened_object.named_object.is_dir() {
            return Err(Errno::EBADF);
        }

        Err(Errno::ENOTSUP)
    })
}

/// `read` data into `buf` from object `handle` opened by process `owner`
pub(super) fn read(owner: Uuid, handle: usize, buf: &mut [u8]) -> Result<usize, Errno> {
    lookup_opened_object(owner, handle).and_then(|opened_object| {
        if opened_object.named_object.is_file() {
            return opened_object.named_object.as_file().and_then(|file| {
                let pos = opened_object.pos.load(Ordering::SeqCst);
                let bytes_read = file.read(buf, pos, opened_object.options)?;
                opened_object.pos.store(pos + bytes_read, Ordering::SeqCst);
                Ok(bytes_read) // Return the bytes read
            });
        }
        if opened_object.named_object.is_pipe() {
            return opened_object.named_object.as_pipe().and_then(|pipe| {
                let bytes_read = pipe.read(buf, 0, opened_object.options)?;
                Ok(bytes_read) // Return the bytes read
            });
        }

        if opened_object.named_object.is_dir() {
            return Err(Errno::EISDIR);
        }

        Err(Errno::ENOTSUP)
    })
}

/// Change position in byte stream of object `handle` opened by process `owner`
pub(super) fn seek(owner: Uuid, fh: usize, offset: isize, origin: SeekOrigin) -> Result<usize, Errno> {
    lookup_opened_object(owner, fh).and_then(|opened_object| {
        if opened_object.named_object.is_file() {
            return opened_object.named_object.as_file().and_then(|file| {
                // base position the offset is relative to
                let base = match origin {
                    SeekOrigin::Start => 0,
                    SeekOrigin::End => file.stat()?.size,
                    SeekOrigin::Current => opened_object.pos.load(Ordering::SeqCst),
                };

                // new position = base + offset; must not become negative or overflow
                // (a negative isize cast to usize would become a huge position)
                let new_pos = base.checked_add_signed(offset).ok_or(Errno::EINVAL)?;
                opened_object.pos.store(new_pos, Ordering::SeqCst);
                Ok(new_pos) // Success
            });
        }
        Err(Errno::ENOTSUP)
    })
}

/// Read next directory entry of directory referenced by `handle` \
pub(super) fn readdir(owner: Uuid, handle: usize) -> Result<Option<DirEntry>, Errno> {
    lookup_opened_object(owner, handle).and_then(|opened_object| {
        if opened_object.named_object.is_dir() {
            return opened_object.named_object.as_dir().and_then(|dir| {
                let pos = opened_object.pos.load(Ordering::SeqCst);
                let dir_entry = dir.readdir(pos)?;
                opened_object.pos.store(pos + 1, Ordering::SeqCst);
                Ok(dir_entry) // Return the DirEntry
            });
        }
        Err(Errno::ENOTSUP)
    })
}

/// Close object `handle` opened by process `owner`
pub(super) fn close(owner: Uuid, handle: usize) -> Result<usize, Errno> {
    // Remove the entry (atomic under the table lock): if two threads close the
    // same handle, only one gets the entry, so a pipe endpoint is closed once.
    let opened_object = remove_opened_object(owner, handle)?;

    // Close the pipe endpoint without holding the table lock
    // (`Pipe::close` wakes waiters via the scheduler).
    opened_object.close_pipe_endpoint();

    Ok(0)
}

/// Reclaim every handle of process `owner` and forget its table, restoring pipe
/// endpoint counts. Called when the process exits. Returns the number of handles
/// reclaimed.
///
/// The table is removed from the map first, then its entries are taken out under
/// the table lock, and the pipe endpoints are closed without holding any lock
/// (`Pipe::close` wakes waiters via the scheduler). A concurrent `close(fh)` of
/// the same process either removed its entry before (and closes it itself) or
/// finds no entry, so an endpoint counter cannot be decremented twice.
pub(super) fn close_all_for_process(owner: Uuid) -> usize {
    info!("close_all_for_process {}", owner);

    let table = tables().write().remove(&owner); // map lock released here
    let Some(table) = table else {
        return 0;
    };

    let victims = table.drain();
    for opened_object in &victims {
        opened_object.close_pipe_endpoint();
    }

    let reclaimed = victims.len();
    if reclaimed > 0 {
        info!("naming: reclaimed {} handles for pid {}", reclaimed, owner);
    }
    reclaimed
}

/// Delete an opened object and free its handle.
/// The handle is freed (and a pipe endpoint closed) even if deleting the object fails;
/// in that case the error of the delete is returned.
pub(super) fn delete(owner: Uuid, fh: usize) -> Result<usize, Errno> {
    debug!("delete called for fh={}", fh);

    // Remove the entry (atomic, see `close`)
    let opened_object = remove_opened_object(owner, fh)?;

    // Delete the object from its parent directory
    let result = opened_object.delete();
    match &result {
        Ok(_) => debug!("  deleted {}", opened_object.name),
        Err(e) => warn!("  error when deleting {}: {:?}", opened_object.name, e),
    }

    // Close the pipe endpoint without holding the table lock
    opened_object.close_pipe_endpoint();

    result.map(|_| 0)
}

pub(super) fn list_dir(owner: Uuid, fh: usize) -> Result<Vec<(NamedObject, String)>, Errno> {
    debug!("lsdir called for fh={}", fh);
    let opened_object = lookup_opened_object(owner, fh)?;
    match opened_object.named_object.as_dir() {
        Ok(dir) => dir.list(),
        Err(_) => Err(Errno::ENOTDIR),
    }
}

/// Table of the objects opened by one process. The handle is the index into
/// `slots`; the lowest free handle is reused first (like POSIX file descriptors).
/// Handles are only valid within the owning process.
struct OpenObjectTable {
    slots: RwLock<Vec<Option<Arc<OpenedObject>>>>,
}

impl OpenObjectTable {
    /// Create a new, empty table (memory is allocated on the first open)
    fn new() -> OpenObjectTable {
        OpenObjectTable { slots: RwLock::new(Vec::new()) }
    }

    /// Lookup an 'OpenedObject' for a given handle
    fn lookup(&self, handle: usize) -> Result<Arc<OpenedObject>, Errno> {
        self.slots
            .read()
            .get(handle)
            .and_then(|slot| slot.clone())
            .ok_or(Errno::EINVALH)
    }

    /// Allocate a new handle for a given 'OpenedObject'
    fn allocate_handle(&self, opened_object: Arc<OpenedObject>) -> Result<usize, Errno> {
        let mut slots = self.slots.write();

        // reuse the lowest free handle
        if let Some(handle) = slots.iter().position(|slot| slot.is_none()) {
            slots[handle] = Some(opened_object);
            return Ok(handle);
        }

        // otherwise grow the table up to the limit
        if slots.len() >= MAX_OPEN_OBJECTS {
            return Err(Errno::ENOHANDLES);
        }
        slots.push(Some(opened_object));
        Ok(slots.len() - 1)
    }

    /// Remove the entry for `handle` and return it; the handle becomes free
    fn remove(&self, handle: usize) -> Result<Arc<OpenedObject>, Errno> {
        let mut slots = self.slots.write();
        let opened_object = slots
            .get_mut(handle)
            .and_then(|slot| slot.take())
            .ok_or(Errno::EINVALH)?;

        // drop free slots at the end, so the table shrinks again
        while matches!(slots.last(), Some(None)) {
            slots.pop();
        }
        Ok(opened_object)
    }

    /// Remove and return all entries. Pipe endpoints are *not* closed here
    /// (see `close_all_for_process`); this only touches the table.
    fn drain(&self) -> Vec<Arc<OpenedObject>> {
        self.slots.write().drain(..).flatten().collect()
    }
}


/// Opened object stored in an 'OpenObjectTable' \
/// (includes NamedObject, current position within object, and options)
pub struct OpenedObject {
    named_object: Arc<NamedObject>,
    name: String,
    pos: AtomicUsize, // current position within file or number of next DirEntry
    options: OpenOptions,
    parent: Option<Arc<dyn DirectoryObject>>, // parent directory
}

impl OpenedObject {
    pub fn new(named_object: Arc<NamedObject>, name: String, pos: AtomicUsize, options: OpenOptions, parent: Option<Arc<dyn DirectoryObject>>) -> OpenedObject {
        OpenedObject { named_object, name, pos, options, parent }
    }

    /// If this is a pipe, release the endpoint taken in `open`
    fn close_pipe_endpoint(&self) {
        if let Ok(pipe) = self.named_object.as_pipe() {
            pipe.close(self.options);
        }
    }

    pub fn delete(&self) -> Result<(), Errno> {
        if let Some(parent_dir) = &self.parent {
            match Arc::as_ref(&self.named_object) {
                NamedObject::FileObject(_) => parent_dir.delete_file(&self.name),
                NamedObject::PipeObject(_) => parent_dir.delete_pipe(&self.name),
                NamedObject::DirectoryObject(_) => parent_dir.delete_dir(&self.name),
            }
        } else {
            // TODO: allow deleting root directories
            Err(Errno::EACCES)
        }
    }
}
