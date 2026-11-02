/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: wait_queue                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Wait queues for blocking i/o.                                           ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - wait:       Blocks calling thread if the given predicate is true.   ║
   ║   - notify_one: Deblocks one waiting thread (if any).                   ║
   ║   - notify_all: Deblocks all waiting threads (if any).                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 16.02.2026               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::{collections::VecDeque, vec::Vec};
use uuid::Uuid;

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
    pub fn wait<F>(&self, mut pred: F, _message: &str)
    where
        F: FnMut() -> bool,
    {
        loop {
            if pred() {
                return;
            }

            let ids = scheduler().park_current();
            {
                let mut guard = self.queue.lock();
                guard.push_back(ids);
            }

            // Blocks only if we are still Parking AND still need to wait. The
            // predicate is re-evaluated under the scheduler lock, which is
            // serialized against `unblock()`, so a wakeup can never be lost.
            scheduler().block_if_parking(|| !pred());
            scheduler().exit_if_killed();

            // Drop our registration if a notify has not already removed it (e.g.
            // when `block_if_parking` returned without blocking due to a race).
            {
                let mut guard = self.queue.lock();
                if let Some(pos) = guard.iter().position(|entry| *entry == ids) {
                    guard.remove(pos);
                }
            }
        }
    }

    /// Wake up exactly one waiter (if any). Returns true if someone was woken up.
    pub fn notify_one(&self) -> bool {
        let mut guard = self.queue.lock();
        
        let mut unblocked_success_idx = None;

        // check queue until one thread is successfully unblocked
        for (idx, (pid, tid)) in guard.iter().enumerate() {
            if scheduler().unblock(*pid, *tid) {
                unblocked_success_idx = Some(idx);
                break;
            }
        }

        match unblocked_success_idx {
            Some(idx) => guard.remove(idx).is_some(),
            None => false
        }
    }

    /// Wake up all waiters currently queued.
    /// Returns the number of threads actually unblocked (stale entries are ignored).
    pub fn notify_all(&self) -> usize {
        let mut guard = self.queue.lock();
        
        let mut unblocked_success_tids = Vec::new();

        // check queue and try to unblock waiting threads
        for (pid, tid) in guard.iter() {
            if scheduler().unblock(*pid, *tid) {
                unblocked_success_tids.push(*tid);
            }
        }

        let prev_len = guard.len();
        // remove successfully unblocked threads from the queue
        guard.retain(|(_pid, tid)| !unblocked_success_tids.contains(tid));
        let num_removed = prev_len - guard.len();

        num_removed
    }
}
