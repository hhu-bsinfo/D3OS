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
use crate::{ process_manager, scheduler};
use crate::memory::pages::Paging;
use crate::memory::vmm::VirtualAddressSpace;

static PROCESS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next_process_id() -> usize {
    PROCESS_ID_COUNTER.fetch_add(1, Relaxed)
}


pub struct Process {
    pub id: usize,
    pub virtual_address_space: VirtualAddressSpace,
}


impl Process {
    pub fn new(page_tables: Arc<Paging>) -> Self {
        Self { id: next_process_id(), virtual_address_space: VirtualAddressSpace::new(page_tables) }
    }

    /// Return the id of the process
    pub fn id(&self) -> usize {
        self.id
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
