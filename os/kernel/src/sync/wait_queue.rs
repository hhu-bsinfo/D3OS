/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: wait_queue                                                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Wait queues for blocking i/o.                                           ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - wait:       Blocks calling thread if the given predicate is true.   ║
   ║   - notify_one: Deblocks one waiting thread (if any).                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 01.09.2025               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::collections::VecDeque;

use crate::scheduler;
use crate::sync::irqsave_spinlock::IrqSaveSpinlock;

pub struct WaitQueue {
    queue: IrqSaveSpinlock<VecDeque<(usize, usize)>>,
}

impl WaitQueue {
    pub fn new() -> WaitQueue {
        WaitQueue {
            queue: IrqSaveSpinlock::new(VecDeque::<(usize,usize)>::new()),
        }
    }

    /// Block until `pred()` becomes true. 
    pub fn wait<F>(&self, mut pred: F)
    where
        F: FnMut() -> bool,
    {
        loop {
            if pred() {
                return;
            }

            let (pid, tid) = scheduler().current_ids();

            // Take the queue lock, and re-check the predicate while holding it to avoid racing with notify_*().
            {
                let mut quard = self.queue.lock(); // IRQs disabled & spinlocked here

                if pred() {
                    // Condition became true while we were getting the lock; don't sleep.
                    return;
                }

                // Enqueue ourselves as a waiter
                quard.push_back((pid, tid));
                // lock is dropped here => IRQs restored
            }

            // Block the current thread, it will be woken by notify_one/notify_all.
            scheduler().block();
            
            // On wake, loop and check pred() again.
        }
    }

    /// Wake exactly one waiter (if any). Returns true if someone was woken.
    pub fn notify_one(&self) -> bool {
        let waiter = {  
            let mut quard = self.queue.lock();
            quard.pop_front()
        };
        if let Some((pid, tid)) = waiter {
            scheduler().deblock(pid, tid);
            true
        } else {
            false
        }
    }


 }
