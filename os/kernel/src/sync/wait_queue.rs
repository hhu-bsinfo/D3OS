/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: wait_queue                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Wait queues for blocking i/o.                                           ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - wait        Blocks calling thread if the given predicate is true.   ║
   ║   - notify_one  Deblocks one waiting thread (if any).                   ║
   ║   - notify_all  Deblocks all waiting threads (if any).                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 18.11.2026               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::{collections::VecDeque, vec::Vec};
use uuid::Uuid;
use log::info;

use crate::{process::core_local_storage::scheduler, sync::irqsave_spinlock::IrqSaveSpinlock};

pub struct WaitQueue {
    queue: IrqSaveSpinlock<VecDeque<(Uuid, usize)>>,
}

impl WaitQueue {
    pub const fn new() -> WaitQueue {
        WaitQueue {
            queue: IrqSaveSpinlock::new(VecDeque::<(Uuid, usize)>::new()),
        }
    }

    /// Block until `pred()` becomes true.
    pub fn wait<F: FnMut() -> bool>(&self, mut pred: F, _message: &str) {
        let ids = scheduler().current_ids();
        loop {
            let mut guard = self.queue.lock();
            if pred() {
                return; // drop guard, release lock
            }
            guard.push_back(ids); // insert caller in wait_queue

            scheduler().block_and_release(guard); // block caller and move guard (released in block_and_release)

            // Ensure caller is removed from wait_queue 
            // If pred is false, it will be inserted in the next loop iteration
            let was_notified = !self.remove(ids);

            // If notified, check it caller should be killed
            if was_notified && scheduler().current_thread().is_kill_requested() {
                // We got a wakeup but are about to die ,so pass it to next thread in wait_queue
                self.notify_one();
            }

            // Check if thread is about to die (independent of real or spurious wakeup)
            scheduler().exit_if_killed();
        }
    }

    /// Remove `ids` tuple from the queue. 
    /// Returns: true if the tuple was found & removed, false otherwise
    fn remove(&self, ids: (Uuid, usize)) -> bool {
        let mut guard = self.queue.lock();
        match guard.iter().position(|entry| *entry == ids) {
            Some(pos) => {
                guard.remove(pos);
                true
            }
            None => false,
        }
    }

    /// Wake up exactly one waiter (if any). 
    pub fn notify_one(&self) {
        if let Some(thread) = self.queue.lock().front() {
            scheduler().unblock(thread.0, thread.1);
        }
    }

    /// Wake up all waiters currently queued
    pub fn notify_all(&self) {
        for (pid, tid) in self.queue.lock().iter() {
            scheduler().unblock(*pid, *tid);
        }
    }
}
