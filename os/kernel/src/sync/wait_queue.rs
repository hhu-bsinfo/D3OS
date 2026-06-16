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

use alloc::collections::VecDeque;
use log::info;

use crate::scheduler;
use crate::sync::irqsave_spinlock::IrqSaveSpinlock;

pub struct WaitQueue {
    queue: IrqSaveSpinlock<VecDeque<(usize, usize)>>,
}

impl WaitQueue {
    pub fn new() -> WaitQueue {
        WaitQueue {
            queue: IrqSaveSpinlock::new(VecDeque::<(usize, usize)>::new()),
        }
    }

    /// Block until `pred()` becomes true.
    pub fn wait<F>(&self, mut pred: F, message: &str)
    where
        F: FnMut() -> bool,
    {
        //        info!("WaitQueue::wait");
        let (pid, tid) = scheduler().current_ids();

        loop {
            if pred() {
                return;
            }

            {
                let mut guard = self.queue.lock();

                // re-check under lock
                if pred() {
                    return;
                }

                guard.push_back((pid, tid));

                // park after we are visible to notifiers
                scheduler().park_current();
            }

            scheduler().yield_now(); 
        }

        info!("WaitQueue::wait: Thread with PID={}, TID={} is now waiting, message = {}", pid, tid, message);

        // Check predicate without acquiring the queue lock.
        // The scheduler will block us, as long as no `notify_one` and `notify_all` have arrived
        // But even after waking up we need to check for spurious wakeups, so we loop here.
        loop {
            if pred() {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Wake up exactly one waiter (if any). Returns true if someone was woken up.
    pub fn notify_one(&self) -> bool {
        //  info!("WaitQueue::notify_one");

        let mut guard = self.queue.lock();

        while let Some((pid, tid)) = guard.pop_front() {
            if scheduler().unblock(pid, tid) {
                // info!("WaitQueue::notify_one: found a waiter");
                return true;
            }
            // else: stale waiter (killed/exited) -> keep going
        }
        //    info!("WaitQueue::notify_one: no waiter found");

        false
    }

    /// Wake up all waiters currently queued.
    /// Returns the number of threads actually unblocked (stale entries are ignored).
    pub fn notify_all(&self) -> usize {
        let mut guard = self.queue.lock();
        let mut woke = 0;

        while let Some((pid, tid)) = guard.pop_front() {
            if scheduler().unblock(pid, tid) {
                woke += 1;
            }
            // else: stale waiter (killed/exited) -> ignore
        }

        woke
    }
}
