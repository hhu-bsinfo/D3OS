/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Implementation of processes.                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, HHU                                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::warn;
use spin::Mutex;
use uuid::{ContextV7, Timestamp, Uuid};
use core::ops::Deref;
use core::sync::atomic::Ordering::Relaxed;
use crate::process::core_local_storage::scheduler;
use crate::{network, now, process_manager, timer};
use crate::memory::pages::Paging;
use crate::memory::vmm::VirtualAddressSpace;
use core::sync::atomic::AtomicU64;

static UUID_CONTEXT: Mutex<ContextV7> = Mutex::new(ContextV7::new());

/// A process contains virtual memory and [`super::thread::Thread`]s.
pub struct Process {
    /// The process ID is a UUID so that its ID is unique across hosts and reboots.
    id: Uuid,
    name: String,
    pub virtual_address_space: VirtualAddressSpace,
    pub utime: AtomicU64,
    pub stime: AtomicU64,
    pub rss_user_pages: AtomicU64, // only userpages, because kernel mapping 1:1
}


impl Process {
    pub fn new(page_tables: Arc<Paging>, name: String) -> Self {
        Self::new_with_id(page_tables, Self::next_id(), name)
    }
    
    pub fn new_kernel(page_tables: Arc<Paging>) -> Self {
        Self::new_with_id(page_tables, Uuid::nil(), "kernel".into())
    }
    
    fn new_with_id(page_tables: Arc<Paging>, id: Uuid, name: String) -> Self {
        Self { id, name,
            virtual_address_space: VirtualAddressSpace::new(page_tables), 
            utime: AtomicU64::new(0), // track the time spent in User-Mode
            stime: AtomicU64::new(0), // track the time spent in Kernel-Mode 
            rss_user_pages: AtomicU64::new(0) } // track the amount of pages allocated
    }
    
    /// Get a new ID for a new process.
    /// 
    /// This being a UUID v7 is an implementation detail, but it is nice to have ascending IDs.
    fn next_id() -> Uuid {
        let timestamp = match now() {
            Some(datetime) => datetime.timestamp_millis().try_into().unwrap(),
            None => {
                warn!("couldn't get current time, using systime");
                timer().systime_ms()
            },
        };
        let seconds = timestamp / 1000;
        let fraction = timestamp % 1000;
        Uuid::new_v7(Timestamp::from_unix(
            UUID_CONTEXT.lock().deref(), seconds.try_into().unwrap(), fraction.try_into().unwrap(),
        ))
    }

    /// Return the id of the process
    pub fn id(&self) -> Uuid {
        self.id
    }
    
    /// Return the name of the process
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Return the utime of the process
    pub fn utime(&self) -> u64 {
        self.utime.load(Relaxed) 
    }

    /// Return the stime of the process
    pub fn stime(&self) -> u64 {
        self.stime.load(Relaxed) 
    }

    /// Return the rss of the process
    pub fn rss_user_pages(&self) -> u64 {
        self.rss_user_pages.load(Relaxed)
    }

    pub fn exit(&self) {
        process_manager().write().exit(self.id);
    }

    /// Return the ids of all threads of the process
    pub fn thread_ids(&self) -> Vec<usize> {
        scheduler().active_thread_ids().iter()
            .filter(|&&thread_id| {
                scheduler().thread(thread_id).is_some_and(|thread| thread.process().id() == self.id)
            }).copied().collect()
    }

    pub fn kill_all_threads_but_current(&self) {
        self.thread_ids().iter()
            .filter(|&&thread_id| thread_id != scheduler().current_thread().id())
            .for_each(|&thread_id| scheduler().kill(thread_id));
    }

    pub fn dump(&self) {
        self.virtual_address_space.dump(self.id);
    }

    // increment utime if in User-Mode, stime if in Kernel-Mode 
    pub fn account_tick(&self, from_user: bool) {
        if from_user {
            // Increase Time in User-Mode
            self.utime.fetch_add(1, Relaxed);
        } else {
            // Increase Time in Kernel-Mode
            self.stime.fetch_add(1, Relaxed);
        }
    }

    // increment rss
    pub fn account_rss(&self) {
        self.rss_user_pages.fetch_add(1, Relaxed);
    }
}

impl PartialEq for Process {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl core::fmt::Debug for Process {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Process").field("id", &self.id).finish()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        network::close_sockets_for_process(self)
    }
}
