/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Implementation of processes.                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, HHU                                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use crate::process::core_local_storage::scheduler;
use crate::{ network, process_manager};
use crate::memory::pages::Paging;
use crate::memory::vmm::VirtualAddressSpace;
use core::sync::atomic::AtomicU64;

static PROCESS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next_process_id() -> usize {
    PROCESS_ID_COUNTER.fetch_add(1, Relaxed)
}


pub struct Process {
    pub id: usize,
    pub virtual_address_space: VirtualAddressSpace,
    pub utime: AtomicU64,
    pub stime: AtomicU64,
    pub rss_user_pages: AtomicU64, // only userpages, because kernel mapping 1:1
}


impl Process {
    pub fn new(page_tables: Arc<Paging>) -> Self {
        Self { id: next_process_id(), 
            virtual_address_space: VirtualAddressSpace::new(page_tables), 
            utime: AtomicU64::new(0), // track the time spent in User-Mode
            stime: AtomicU64::new(0), // track the time spent in Kernel-Mode 
            rss_user_pages: AtomicU64::new(0) } // track the amount of pages allocated
    }

    /// Return the id of the process
    pub fn id(&self) -> usize {
        self.id
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
