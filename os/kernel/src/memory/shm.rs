/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: shm                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Public functions related to shared memory.                              ║
   ║   - open          get id of shared memory region or create a new one    ║
   ║   - attach        attach shared memory region into virtual adress space ║ 
   ║   - detach        detach shared memory region from virtual adress space ║
   ║   - unlink        unlink shared memory region                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Laurenz Maslo, Univ. Duesseldorf, 26.01.2026                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use core::{sync::atomic::{AtomicBool, AtomicUsize, Ordering}, usize};
use syscall::return_vals::Errno;
use x86_64::structures::paging::{frame::PhysFrameRange};
use alloc::{collections::BTreeMap, string::{String}};
use alloc::sync::Arc;
use spin::{Once, RwLock};
use crate::{consts::MAX_SHM_SIZE, memory::{MemorySpace, PAGE_SIZE, vma::VmaType, vmm}, process_manager};

struct SharedMemoryEntry {
    size:usize,
    frames:PhysFrameRange,
    counter:AtomicUsize,
    unlinked:AtomicBool
}

impl SharedMemoryEntry {
    pub fn new(size: usize, frames: PhysFrameRange) -> Self {

        return Self {
            size,
            frames,
            counter: AtomicUsize::new(0),
            unlinked: AtomicBool::new(false)
        }
    }
}

struct ShmTables {
    naming_table: BTreeMap<String, usize>,
    entry_table: BTreeMap<usize, Arc<SharedMemoryEntry>>,
    next_free_id: AtomicUsize
}

impl ShmTables {
    pub fn new() -> Self {
        return Self {
            naming_table: BTreeMap::new(),
            entry_table: BTreeMap::new(),
            next_free_id: AtomicUsize::new(0)
        }
    }
}

static SHM_TABLES: Once<RwLock<ShmTables>> = Once::new();

fn shm_tables() -> &'static RwLock<ShmTables> {
    SHM_TABLES.call_once(|| RwLock::new(ShmTables::new()))
}


pub fn open(name: String, size:usize, create: bool) -> isize {
    // lock table here to make open atomic in relation to other shm functions
    let mut table = shm_tables().write();

    // Param validation
    if size == 0 || size > MAX_SHM_SIZE {
        return Errno::EINVAL.into();
    }

    if create {
        //info!("Anazhl shm: {}", table.entry_table.len());
        if table.naming_table.contains_key(&name) {
            return Errno::EEXIST.into();
        }

        // allocate enough frames for size
        let frame_count = (size + PAGE_SIZE - 1) / PAGE_SIZE;
        let frames = unsafe { vmm::alloc_frames(frame_count) };

        // generate id for shm entry
        let id = table.next_free_id.fetch_add(1, Ordering::SeqCst);

        // create entry and insert into tables
        let entry = SharedMemoryEntry::new( frame_count * PAGE_SIZE, frames);
        table.entry_table.insert(id, Arc::new(entry));
        table.naming_table.insert(name, id);

        return id as isize;
    }
    else {
        // get id from naming_table
        let id = match table.naming_table.get(&name) {
            Some(id) => id.clone(),
            None => return Errno::ENOENT.into()
        };

        // Error if size > entry.size
        match table.entry_table.get(&id) {
            Some(entry) => if size > entry.size { return Errno::EINVAL.into() },
            None => return Errno::ENOENT.into()
        }

        return id as isize;
    }
}

pub fn attach(id: usize, read_only: bool) -> isize {
    let process = process_manager().read().current_process();

    // lock table here to make attach atomic in relation to other shm functions
    let table = shm_tables().write();

    let entry = match table.entry_table.get(&id) {
        Some(e) => e,
        None => return Errno::EINVAL.into()
    };

    // allocate vma for current process (error if no vma created)
    let num_pages = entry.size / PAGE_SIZE;
    let vma_type = VmaType::SharedMemory { id };
    let vma = match process.virtual_address_space.alloc_vma(Option::None, num_pages as u64, MemorySpace::User, vma_type, "shm") {
            Some(vma) => vma,
            None => return Errno::ENOMEM.into()
    };


    // create mapping in page table
    let flags = match read_only { 
        true => x86_64::structures::paging::PageTableFlags::PRESENT | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE,
        false => x86_64::structures::paging::PageTableFlags::PRESENT | x86_64::structures::paging::PageTableFlags::WRITABLE | x86_64::structures::paging::PageTableFlags::USER_ACCESSIBLE
    };
    match process.virtual_address_space.map_pfr_for_vma(&vma, entry.frames, flags) {
        Ok(_) => (),
        Err(_) => { process.virtual_address_space.unmap_vma(vma, false); return Errno::ENOMEM.into() } // delete vma if mapping fails
    };

    // increase counter
    entry.counter.fetch_add(1, Ordering::SeqCst);

    // first virtual adress of shm
    return vma.start().as_u64() as isize;
}

pub fn detach(ptr:*mut u8) -> isize {
    let process = process_manager().read().current_process();

    // lock table here to make detach atomic in relation to other shm functions
    let table = shm_tables().write();

    // get vma of current process for pointer (error if no vma for pointer)
    let vma = match process.virtual_address_space.is_address_within_vma(ptr as u64, VmaType::SharedMemory { id: 0 }) {
        Some(vma) => vma,
        None => return Errno::EINVAL.into()
    };

    // get shm_id from vma (error if vma not of type shm)
    let id = if let VmaType::SharedMemory { id} = vma.typ { id } else { return Errno::EINVAL.into(); };

    // remove vma from virtual adress space of process
    process.virtual_address_space.unmap_vma(vma, false);

    let entry = match table.entry_table.get(&id) {
        Some(e) => e,
        None => return Errno::EINVAL.into()
    };

    // decrease counter
    let old_counter = entry.counter.fetch_sub(1, Ordering::SeqCst);
    
    // delete if unlinked and no process attached
    if old_counter == 1 && entry.unlinked.load(Ordering::SeqCst) {
        drop(table);
        delete(id);
    }

    return 0;
}

pub fn unlink(name: String) -> isize {
    //info!("before unlink");
    // lock table here to make unlink atomic in relation to other shm functions
    let mut table = shm_tables().write();

    // remove from naming_table (error if no id for name exists)
    let id = match table.naming_table.remove(&name) {
        Some(id) => id,
        None => return Errno::ENOENT.into()
    };

    let entry = match table.entry_table.get(&id) {
        Some(e) => e.clone(),
        None => return Errno::ENOENT.into()
    };

    // set unlinked flag
    entry.unlinked.store(true, Ordering::SeqCst);

    // delete if no one connected
    if entry.counter.load(Ordering::SeqCst) == 0 {
        drop(table);
        delete(id);
    }
    //info!("unlinked");

    return 0;
}

fn delete(id: usize) {
    let mut table = shm_tables().write();
    // remove entry from entry_table
    let entry = match table.entry_table.remove(&id) {
        Some(entry) => entry,
        None => return // possible because lock got dropped but not a problem
    };

    // free physical frames
    let frames = entry.frames;
    unsafe { vmm::free_frames(frames) };
    //info!("SHM Frames freed");
}