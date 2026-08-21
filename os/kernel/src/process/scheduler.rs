/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: scheduler                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Implementation of a basic round-robin scheduler.                        ║
   ║                                                                         ║
   ║ Public functions                                                        ║
   ║   - active_thread_ids      get a list of all active thread IDs          ║
   ║   - current_thread         get the currently running thread             ║
   ║   - current_ids            get the (pid, tid) of the current thread     ║
   ║   - exit                   exit the calling thread                      ║
   ║   - join                   wait for a thread to finish                  ║
   ║   - kill                   kill a thread                                ║
   ║   - set_init               set the scheduler as initialized             ║
   ║   - thread                 get reference to a thread                    ║
   ║   - ready                  insert a thread in the ready queue           ║
   ║   - sleep                  put the caller into sleeping mode            ║
   ║   - start                  start the scheduler                          ║
   ║   - switch_thread_from_interrupt  switch thread, called from interrupt  ║
   ║   - switch_thread_no_interrupt    switch thread, not called from int.   ║
   ║   - current_ids            get the (pid, tid) of the current thread     ║
   ║   - block                  put the calling thread into blocked mode     ║
   ║   - deblock                wake up a blocked thread                     ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, 05.09.2025, HHU                                 ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::process::thread::{Thread, ThreadState};
use crate::{allocator, apic, per_cpu_ref, timer, tss};
use alloc::collections::VecDeque;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::{vec};
use alloc::vec::Vec;
use syscall::return_vals::Errno;
use uuid::Uuid;
use core::{panic, ptr};
use core::arch::asm;
use core::cell::Cell;
use core::fmt::Write;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use core::sync::atomic::AtomicU32;
use core::sync::atomic::Ordering::{Relaxed};
use log::{debug, info};
use smallmap::Map;
use spin::{Mutex, MutexGuard, Once};
use thingbuf::mpsc::{Sender};
use crate::device::apic::get_apic_id;
use crate::device::cpu::{disable_int_nested, enable_int_nested};
use crate::process::core_local_storage::{cls, current_core_id, scheduler, tss_static};

// thread IDs
pub static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);
static ACTIVE_CPUS: AtomicU32 = AtomicU32::new(1);  //BP automatically

/// Global set of "alive" thread IDs (across all cores).
/// Presence means: joining on this tid should block (unless it exits concurrently).
static ACTIVE_TIDS: Once<Mutex<Map<usize, ()>>> = Once::new();

#[inline]
pub fn active_tids() -> &'static Mutex<Map<usize, ()>> {
    ACTIVE_TIDS.call_once(|| Mutex::new(Map::new()))
}

#[inline]
fn mark_thread_alive(tid: usize) {
    let mut set = active_tids().lock();
    set.insert(tid, ());
}

#[inline]
fn mark_thread_dead(tid: usize) {
    let mut set = active_tids().lock();
    set.remove(&tid);
}

#[inline]
pub fn is_thread_alive(tid: usize) -> bool {
    active_tids().lock().contains_key(&tid)
}

#[inline]
pub fn next_thread_id() -> usize {
    THREAD_ID_COUNTER.fetch_add(1, Relaxed)
}

#[inline]
pub fn cpu_mark_online() {
    ACTIVE_CPUS.fetch_add(1, Relaxed);
}

#[inline]
pub fn cpu_count() -> u32 {
    ACTIVE_CPUS.load(Relaxed)
}

/// Everything related to the threads in ready state in the scheduler
pub struct ReadyState {
    initialized: bool,
    last_fpu_thread: Option<Arc<Thread>>,
    ready_queue: VecDeque<Arc<Thread>>,
    idle_thread: Arc<Thread>
}

impl ReadyState {
    pub fn new() -> Self {

        let initialized = false;
        let ready_queue = VecDeque::new();
        let idle_thread = Thread::new_kernel_thread(idle_thread, "idle");

        Self {
            initialized,
            last_fpu_thread: None,
            ready_queue,
            idle_thread,
        }
    }
}

/// Main struct of the scheduler
pub struct Scheduler {
    current_thread: Cell<Option<Arc<Thread>>>,
    ready_state: Mutex<ReadyState>,
    sleep_list: Mutex<Vec<(Arc<Thread>, usize)>>,
    blocked_list: Mutex<Vec<Arc<Thread>>>,
    join_map: Mutex<Map<usize, Vec<Arc<Thread>>>>, // manage which threads are waiting for a thread-id to terminate
    has_started: bool,
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

/// Called from assembly code, after the thread has been switched
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unlock_scheduler() {
    unsafe { scheduler().ready_state.force_unlock(); }
}

impl Scheduler {

    /// Create and initialize the scheduler.
    pub fn new() -> Self {
        info!("Initializing scheduler for CPU {}", current_core_id());
        let vec = Vec::new();
        let rs = ReadyState::new();

        let ready_state = Mutex::new(rs);
        let sleep_list = Mutex::new(vec);
        let blocked_list = Mutex::new(Vec::new());
        let join_map = Mutex::new(Map::new());
        let has_started = false;

        Self {
            current_thread: Cell::default(),
            ready_state,
            sleep_list,
            blocked_list,
            join_map,
            has_started,
        }
    }

    /// Called after the scheduler has been fully initialized
    pub fn set_init(&self) {
        self.get_ready_state().initialized = true;
    }

    /// returns the number of threads that are currently actively running on this CPU
    /// does not count sleeping threads
    pub fn active_thread_count(&self) -> usize {
        let mut sum: u32 = 0;
        let active = ACTIVE_CPUS.load(Relaxed);
        for i in 0..active {
            let rq = per_cpu_ref(i as usize).rq_len.load(Ordering::Acquire);
            // Detect runaway counters without panicking the whole kernel in debug
            match sum.checked_add(rq) {
                Some(s) => sum = s,
                None => {
                    log::error!("active_thread_count overflow while adding cpu {} rq_len={}", i, rq);
                    return usize::MAX; // saturate to a sentinel
                }
            }
        }

        // Clamp to usize on 32-bit safely (and give a clear sentinel on 32-bit if it ever overflows)
        if sum > usize::MAX as u32 {
            log::error!("active_thread_count exceeds usize: {}", sum);
            usize::MAX
        } else {
            sum as usize
        }
    }


    /// Get all active thread IDs
    pub fn active_thread_ids(&self) -> Vec<usize> {
        Vec::from_iter(active_tids().lock().iter().cloned().map(|t | t.0))
    }

    /// Return the current running thread
    pub fn current_thread(&self) -> Arc<Thread> {
        self.try_current_thread()
            .expect("Trying to access current thread before initialization!")
    }

    /// Return reference to current thread, if possible.
    pub fn try_current_thread(&self) -> Option<Arc<Thread>> {
        self.current_thread
            .get_cloned()
            .take()
    }

    /// Try to return reference to current thread (called from interrupt dispatcher)
    pub fn try_get_current_thread(&self) -> Option<Arc<Thread>> {
        if allocator().is_locked() {
            return None;
        }
        self.try_current_thread()
    }

    /// Return reference to thread identified by `thread_id`
    pub fn thread(&self, thread_id: usize) -> Option<Arc<Thread>> {
        self.ready_state.lock().ready_queue
            .iter()
            .find(|thread| thread.id() == thread_id)
            .cloned()
    }

    /// Return (pid, tid) of current thread
    pub fn current_ids(&self) -> (Uuid, usize) {
        let tid = self.current_thread().id();
        let pid = self.current_thread().process().id();
        (pid, tid)
    }


    /// Start the scheduler, called only once from `boot.rs`
    pub fn start(&mut self) {
        if self.has_started {
            return;
        }
        self.has_started = true;
        let mut state = self.get_ready_state();
        let next_thread = state.ready_queue.pop_back()
            .unwrap_or_else(|| state.idle_thread.clone());
        let old = self.current_thread.replace(Some(next_thread.clone()));
        assert!(old.is_none());

        unsafe { Thread::start_first(next_thread.as_ref()); }
    }

    /// Insert `thread` into the ready queue of the scheduler
    pub fn ready(&self, thread: Arc<Thread>) {
        let id = thread.id();
        mark_thread_alive(id);

        // If we get the lock on 'self.state' but not on 'self.join_map' the system hangs.
        // The scheduler is not able to switch threads anymore, because of 'self.state' is locked,
        // and we will never be able to get the lock on 'self.join_map'.
        // To solve this, we need to release the lock on 'self.state' in case we do not get
        // the lock on 'self.join_map' and let the scheduler switch threads until we get both locks.
        let (mut state, mut join_map) = loop {
            let state = self.get_ready_state();
            if let Some(join_map) = self.join_map.try_lock() {
                break (state, join_map);
            }
            self.switch_thread_no_interrupt();
        };

        inc_rq_len();
        state.ready_queue.push_front(thread);

        // Don't override value if key present
        if !join_map.contains_key(&id){
            join_map.insert(id, Vec::new());
        }
    }

    /// Put calling thread to sleep for `ms` milliseconds
    pub fn sleep(&self, ms: usize) {
        let state = self.get_ready_state();

        if !state.initialized {
            // Scheduler is not initialized yet, so this function has been called during the boot process
            // So we do active waiting
            timer().wait(ms);
        }
        else {
            // Scheduler is initialized, so we can block the calling thread
            let thread = self.current_thread();
            thread.set_state(ThreadState::Sleeping);
            
            {
                // Execute in own block, so that the lock is released automatically (block() does not return)
                let wakeup_time = timer().systime_ms() + ms;
                let mut sleep_list = self.sleep_list.lock();
                sleep_list.push((thread, wakeup_time));
            }

            dec_rq_len();
            self.block_and_switch(state);
        }
    }

    /// Put calling thread to block
    pub fn block(&self) {
        let state = self.get_ready_state();

        if !state.initialized {
            // Scheduler is not initialized yet, so this function has been called during the boot process
            // We panic
            panic!("Scheduler: Cannot block thread before scheduler is initialized!");
        }
        else {
            // Scheduler is initialized, so we can block the calling thread
            let thread = self.current_thread();
            thread.set_state(ThreadState::Blocked);
            {
                // Execute in own block, so that the lock is released automatically (block() does not return)
                let mut block_list = self.blocked_list.lock();
                block_list.push(thread);
            } // drop lock for block_list
            //info!("Scheduler::block: switch to next thread");
            dec_rq_len();
            self.block_and_switch(state);
        }
    }

    /// Requeue thread with `tid` from process with `pid` to the ready queue of the scheduler
    pub fn deblock(&self, pid: Uuid, tid: usize) {
        let mut block_list = self.blocked_list.lock();

        if let Some(pos) = block_list.iter().position(|thread| thread.id() == tid && thread.process().id() == pid) {
            let thread = block_list.remove(pos);
            self.ready(thread);
        } else {
            schedule_on_all_others(MessageItem::Cmd(MessageCmd::Deblock {pid, tid}))
        }
    }

    /// Helper function for switching a thread not caused by an interrupt
    pub fn switch_thread_no_interrupt(&self) {
        self.switch_thread(false);
    }

    /// Helper function for switching a thread caused by an interrupt
    pub fn switch_thread_from_interrupt(&self) {
        self.switch_thread(true);
    }

    /// Calling thread will block until thread with `thread_id` has terminated
    pub fn join(&self, thread_id: usize) -> Result<usize, Errno> {
        // Fast path => if it's already dead, don't block.
        if !is_thread_alive(thread_id) {
            return Err(Errno::ESRCH);
        }

        let state = self.get_ready_state();
        let thread = self.current_thread();
        thread.set_state(ThreadState::Blocked);
        {
            // Execute in own block, so that the lock is released automatically (block() does not return)
            let mut join_map = self.join_map.lock();
            if !is_thread_alive(thread_id) {
                return Err(Errno::ESRCH);
            }
            if let Some(join_list) = join_map.get_mut(&thread_id) {
                join_list.push(thread);
            } else {
                // there is a Map on another Core, but we need one here as well
                join_map.insert(thread_id, vec![thread]);
            }
        }

        dec_rq_len();
        self.block_and_switch(state);
        Ok(0)
    }

    fn unjoin(&self, thread_id: usize, ready_state: &mut ReadyState) {
        let mut join_map = self.join_map.lock();

        if let Some(join_list) = join_map.get_mut(&thread_id) {
            for thread in join_list {
                thread.set_state(ThreadState::Running);
                ready_state.ready_queue.push_front(Arc::clone(thread));
                inc_rq_len();
            }
        }
        schedule_on_all_others(MessageItem::Cmd(MessageCmd::JoinTargetExited {tid: thread_id}));
        join_map.remove(&thread_id);
    }

    /// Exit calling thread.
    pub fn exit(&self) -> ! {
        let mut ready_state = self.get_ready_state();
        let current = self.current_thread();
        current.set_state(ThreadState::Exited);

        // Mark dead globally *before* waking joiners, so joiners racing in will observe "dead"
        mark_thread_dead(current.id());
        self.unjoin(current.id(), &mut ready_state);

        dec_rq_len();
        drop(current); // Decrease Rc manually, because block() does not return
        self.block_and_switch(ready_state);
        unreachable!()
    }

    /// Kill the thread with the id `thread_id`, if it is on the same Core
    pub fn kill(&self, thread_id: usize) {
        let current = self.current_thread();

        // Check if current_thread tries to kill itself (illegal)
        if current.id() == thread_id {
            panic!("A thread cannot kill itself!");
        }

        if self.kill_locally(thread_id, &mut self.get_ready_state()) == false {
            schedule_on_all_others(MessageItem::Cmd(MessageCmd::Kill {tid: thread_id}))
        }
    }

    /// Kill the thread with the id `thread_id`, if it is on the same Core
    /// goes through ready_queue, sleep_list, blocked_list, and join_map in this order
    /// returns true if a thread with the given id was found
    fn kill_locally(&self, thread_id: usize, state: &mut ReadyState) -> bool {
        if is_thread_alive(thread_id) == false { return true; }
        let mut changed = false;

        // check ready_queue
        let mut before = state.ready_queue.len();
        state.ready_queue.retain(|thread| thread.id() != thread_id);
        let mut after = state.ready_queue.len();
        if before != after {
            changed = true;
        }
        if !changed {
            {   // check sleep_list
                let mut sleep_list = self.sleep_list.lock();
                before = sleep_list.len();
                sleep_list.retain(|(thread, _)| thread.id() != thread_id);
                after = state.ready_queue.len() + sleep_list.len();
            }
            if before != after {
                changed = true;
            }
            if!changed {
                {   // check Block List
                    let mut blocked_list = self.blocked_list.lock();
                    let before = blocked_list.len();
                    blocked_list.retain(|t| t.id() != thread_id);
                    if blocked_list.len() != before {
                        changed = true;
                    }
                }
                if !changed {
                    // check all join_map's
                    let mut join_map = self.join_map.lock();
                    for (_target, wait_list) in join_map.iter_mut() {
                        let before = wait_list.len();
                        wait_list.retain(|t| t.id() != thread_id);
                        if wait_list.len() != before {
                            changed = true;
                        }
                    }
                }
            }
        }
        if changed {
            mark_thread_dead(thread_id);
            self.unjoin(thread_id, state);
            dec_rq_len();
        }
        changed
    }

    /// Gives out current thread id, then calls other debug methods
    pub fn debug_scheduler(&self) {
        let state = self.get_ready_state();
        let sleep_list = self.sleep_list.lock();

        let nested = disable_int_nested();
        let id = current_core_id();
        let nbr_threads = self.active_thread_count() as u32;
        let own_threads = read_rq_len() as u32;
        let nbr_cpus = ACTIVE_CPUS.load(Relaxed);
        info!("Scheduler {}: Current thread: {}", id, self.current_thread().id());
        info!("Scheduler{}: total_threads: {}, own_threads: {}, cpus: {}",
                id, nbr_threads, own_threads, nbr_cpus);
        info!("Scheduler {}: Ready queue:", id);
        for thread in &state.ready_queue {
            info!("  - {}", thread.id());
        }
        info!("Scheduler {}: Sleep list:", id);
        for thread in sleep_list.iter() {
            info!("  - {}, {}", thread.0.id(), thread.1);
        }
        for i in 0..nbr_cpus {
            info!("Cpu {} has {} active threads running", i, read_rq_len_remote(i as usize));
        }
        enable_int_nested(nested);
    }

    /// Debugging function to print all threads in the ready queue.
    pub fn debug_ready_queue(&self) {
        let state = self.get_ready_state();
        let id = current_core_id();
        info!("Scheduler {}: Ready queue:", id);
        for thread in &state.ready_queue {
            info!("  - {}", thread.id());
        }
    }

    /// Debugging function to print all threads in the sleep list.
    pub fn debug_sleep_list(&self) {
        let _state_guard = self.get_ready_state();
        let sleep_list = self.sleep_list.lock();
        let id = current_core_id();
        info!("Scheduler {}: Sleep list:", id);
        for thread in sleep_list.iter() {
            info!("  - {}, {}", thread.0.id(), thread.1);
        }
    }

    /// Block calling thread and switch to next ready thread.
    fn block_and_switch(&self, mut state: MutexGuard<ReadyState>) {
        let mut next_thread = state.ready_queue.pop_back();

        if next_thread.is_none() {
            // Execute in own if-block, so that the lock is released automatically (block() does not return)
            let mut sleep_list = self.sleep_list.lock();
            Scheduler::check_sleep_list(&mut state, &mut sleep_list);
            drain_inbox_into_ready(10, &mut state);
            next_thread = state.ready_queue.pop_back();
            if next_thread.is_none() {  //still no new thread => switch to idle
                next_thread = Some(Arc::clone(&state.idle_thread));
            }
        }

        let current = self.current_thread.take()
            .expect("failed to get current thread");
        let next = next_thread.unwrap();

        // Thread has enqueued itself into sleep list and waited so little,
        // that it dequeued itself in the meantime
        if current.id() == next.id() {
            // put it back in
            self.current_thread.set(Some(current));
            return;
        }

        let current_ptr = ptr::from_ref(current.as_ref());
        let next_ptr = ptr::from_ref(next.as_ref());

        self.current_thread.set(Some(next));
        drop(current); // Decrease Rc manually, because Thread::switch does not return

        unsafe {
            Thread::switch(current_ptr, next_ptr);
        }
    }

    /// Prepare to block the calling thread
    /// Used from wait_queue to prepare the thread for blocking and get its (pid, tid) for later `notify_one` and `notify_all` calls
    /// Returns (pid, tid)
    pub fn park_current(&self) -> (Uuid, usize) {
        let thread = self.current_thread();
        thread.set_state(ThreadState::Parking);
        (thread.process().id(), thread.id())
    }

    /// Block the calling thread, but only if it should still wait.
    pub fn block_if_parking<F>(&self, mut should_wait: F)
    where
        F: FnMut() -> bool,
    {
        let state = self.get_ready_state();

        if !state.initialized {
            return;
        }

        let thread = self.current_thread();
        if thread.state() != ThreadState::Parking {
            // A wakeup raced in between registering and blocking; do not block.
            return;
        }

        if !should_wait() {
            // The condition was satisfied while we were registering; a racing
            // `unblock` may not have found us, so cancel the block ourselves.
            thread.set_state(ThreadState::Running);
            return;
        }

        thread.set_state(ThreadState::Blocked);

        {
            let mut block_list = self.blocked_list.lock();
            block_list.push(Arc::clone(&thread));
        }
        dec_rq_len();
        self.block_and_switch(state);
    }

    /// Unblock thread with given (pid, tid). \
    /// Returns true if thread was found and unblocked, false otherwise.
    pub fn unblock(&self, pid: Uuid, tid: usize) -> bool {
       // info!("Unblock: Thread with PID={}, TID={}", pid, tid);

        // Synchronize against `thread_switch`
        let mut state = self.ready_state.lock();

        // 1) Check if the given thread is in the blocked list -> need to be woken up
        let blocked_thread: Option<Arc<Thread>> = {
            let mut block_list = self.blocked_list.lock();
            if let Some(pos) = block_list.iter().position(|t| t.id() == tid && t.process().id() == pid) {
                Some(block_list.remove(pos))
            } else {
                None
            }
        };

        // If we found a blocked thread in the block_list, wake it up
        if let Some(thread) = blocked_thread {
            // let mut state = self.get_ready_state();
            thread.set_state(ThreadState::Ready);
            state.ready_queue.push_front(Arc::clone(&thread));
            inc_rq_len();
            return true;
        }

        // 2a) Check if the thread to be woken up is the current thread (it has not been blocked)
        {
            let curr_thread = self.current_thread();
            if curr_thread.id() == tid && curr_thread.process().id() == pid {
                curr_thread.set_state(ThreadState::Running);
                return true;
            }

        // 2b) Check if the thread to be woken up is in the ready queue
            if state.ready_queue.iter().any(|t| t.id() == tid && t.process().id() == pid) {
                // Already runnable (e.g. a previous wakeup raced in); nothing to do
                return true;
            }
        }

        // 3) Not found on this core. The thread may be blocked on the
        // `blocked_list` of another core (the scheduler is core-local), so ask
        // every other core to deblock it. The owning core's `Deblock` handler
        // does the matching `inc_rq_len()`. Only broadcast for live threads to
        // avoid waking stale (exited) waiters.
        if is_thread_alive(tid) {
            drop(state); // release ready_state before cross-core scheduling
            schedule_on_all_others(MessageItem::Cmd(MessageCmd::Deblock { pid, tid }));
            return true;
        }

        false
    }

    /// Switch from current to next thread (from ready queue). \
    /// If `interrupt` is true, the function is called from an ISR and will send EOI to APIC otherwise not.
    fn switch_thread(&self, interrupt: bool) {
        if let Some(mut state) = self.ready_state.try_lock() {
            if !state.initialized {
                if interrupt { apic().end_of_interrupt(); }
                return;
            }

            // Check for new threads in the sleep list and inbox
            if let Some(mut sleep_list) = self.sleep_list.try_lock() {
                Scheduler::check_sleep_list(&mut state, &mut sleep_list);
            }
            drain_inbox_into_ready(10, &mut state);

            // Check if this core has too many threads running
            if read_resched_flag() || self.should_balance_now() {
                self.balance_once(&mut state);
            }

            // Get clone of the current thread
            let current = self.current_thread();
            let current_was_idle = current.id() == state.idle_thread.id();

            // Current thread is initializing itself and may not be interrupted
            if current.stacks_locked() || tss_static().is_locked() {
                if interrupt {
                    apic().end_of_interrupt();
                }
                return;
            }

            // Try to get the next thread from the ready queue
            let next = match state.ready_queue.pop_back() {
                Some(thread) => thread,
                None => {
                    if interrupt {
                        apic().end_of_interrupt();
                    }
                    //no new thread & idle thread already active => nothing to do
                    if current_was_idle {
                        return;
                    }
                    //no new thread & last!=idle => switch to idle
                    Arc::clone(&state.idle_thread)
                },
            };

            let current_ptr = ptr::from_ref(current.as_ref());
            let next_ptr = ptr::from_ref(next.as_ref());

            self.current_thread.set(Some(next));

            // last!=idle => we need to enqueue it back in the readyQueue
            if current_was_idle == false {
                state.ready_queue.push_front(current);
            }

            if interrupt {
                apic().end_of_interrupt();
            }

            unsafe {
                Thread::switch(current_ptr, next_ptr);
            }
        } else {
            if interrupt {
                apic().end_of_interrupt();
            }
        }
    }

    pub fn switch_fpu_context(&self) {
        let mut state = self.ready_state.lock();
        let current = self.current_thread();

        unsafe { asm!("clts"); }

        if state.last_fpu_thread.is_some() {
            let last = state.last_fpu_thread.as_ref().unwrap();
            last.store_fpu_context();

            if current.id() != last.id() {
                last.store_fpu_context();
            }

            current.restore_fpu_context();
        }

        state.last_fpu_thread = Some(current);
    }

    /// Checks whether the current core should balance its threads.
    /// returns own_threads > (nbr_threads /nbr_cpus +1)
    fn should_balance_now(&self) -> bool {
        let nbr_threads = self.active_thread_count() as u32;
        let own_threads = read_rq_len() as u32;
        let nbr_cpus = ACTIVE_CPUS.load(Relaxed);
        if own_threads > (nbr_threads /nbr_cpus +1){
            /*debug!("Scheduler{}: total_threads: {}, own_threads: {}, cpus: {}",
                current_core_id(), nbr_threads, own_threads, nbr_cpus);*/
            return true }
        false
    }

    /// Balances the threads on the current core by moving one thread from the tail to the target core.
    /// Target core is the core with the least number of threads.
    /// Returns the new state of the scheduler. (needed for mutable access)
    fn balance_once(&self, state: &mut ReadyState) {
        let own_load = read_rq_len() as usize;
        if own_load <= 1 {
            //debug!("Scheduler: Cannot balance, current load ({:?}) is too low!", own_load);
            return;
        }

        if let Some((target_core, target_load)) = self.find_less_loaded_core() {
            if own_load >= target_load + 2 {
                let amount = ((own_load-target_load)/4)+1;
                for _ in 0..amount {
                    // Move one thread from the tail to the target
                    let thread_opt = state.ready_queue.pop_front();
                    if let Some(thread) = thread_opt {
                        // If we can't migrate this thread, skip it.
                        // This makes us migrate one less thread than we wanted,
                        // but this should be okay in general.
                        if !thread.can_migrate() {
                            info!("cannot migrate {thread:?}, skipping");
                            state.ready_queue.push_back(thread);
                            continue;
                        }

                        // Check if the migrating thread is the last thread on this core that has used the FPU.
                        // If so, we need to reset `last_fpu_thread` to None.
                        // We do not need to store the FPU context of the migrating thread,
                        // as we always take a thread from the ready queue and never the current thread.
                        if state.last_fpu_thread.is_some() {
                            if state.last_fpu_thread.as_ref().unwrap().id() == thread.id() {
                                state.last_fpu_thread = None;
                            }
                        }
                        let _tid = thread.id();
                        let w = MessageItem::new_thread(thread);
                        dec_rq_len();
                        if let Ok(_r) = schedule_on(target_core, w) {
                            debug!(" Scheduler{}: Scheduled thread {} on core {}", current_core_id(), _tid, target_core);
                        }
                    }
                }
            }
        }
    }

    /// Finds the core with the least number of threads.
    /// returns (target_core, target_load)
    fn find_less_loaded_core(&self) -> Option<(usize, usize)> {
        // Inspect per-core exported metrics
        let mut curr: usize = 0;
        let mut min = read_rq_len_remote(0);
        for i in 1..ACTIVE_CPUS.load(Relaxed) as usize {
            if min > read_rq_len_remote(i) {
                min = read_rq_len_remote(i);
                curr = i;
            }
        }
        let own = read_rq_len();
        if min >= own { return None; }
        Some((curr, min as usize))
    }

    /// Finds the core with the most number of threads.
    /// returns the target's Core Id
    fn find_more_loaded_core(&self) -> Option<usize> {
        // Inspect per-core exported metrics
        let mut curr: usize = 0;
        let mut max = read_rq_len_remote(0);
        for i in 1..ACTIVE_CPUS.load(Relaxed) as usize {
            if max < read_rq_len_remote(i) {
                max = read_rq_len_remote(i);
                curr = i;
            };
        }
        let own = read_rq_len();
        if max <= own || max < 2 { return None; }
        Some(curr)
    }

    /// Forces a core with more than 2 threads to migrate one through a Reschedule IPI.
    /// (Sends a reschedule IPI that will result in a migration within switch_thread())
    pub fn look_for_overloaded_core(&self) {
        let overloaded_core = self.find_more_loaded_core();
        match overloaded_core {
            None => return,
            Some(target_id) => send_reschedule_ipi(target_id)
        }
    }

    

    /// Check the sleep list for threads that need to be woken up
    fn check_sleep_list(state: &mut ReadyState, sleep_list: &mut Vec<(Arc<Thread>, usize)>) {
        let time = timer().systime_ms();

        sleep_list.retain(|entry| {
            if time >= entry.1 {
                state.ready_queue.push_front(Arc::clone(&entry.0));
                inc_rq_len();
                false
            } else {
                true
            }
        });
    }

    /// Helper function returning `ReadyState` of scheduler in a MutexGuard
    fn get_ready_state(&self) -> MutexGuard<'_, ReadyState> {
        let state;

        // We need to make sure, that both the kernel memory manager and the ready queue are currently not locked.
        // Otherwise, a deadlock may occur: Since we are holding the ready queue lock,
        // the scheduler won't switch threads anymore, and none of the locks will ever be released
        loop {
            let state_tmp = self.ready_state.lock();
            if allocator().is_locked() {    //allocator can be locked again, but only on other cores -> no deadlock, but bottleneck
                continue;
            }

            state = state_tmp;
            break;
        }

        state
    }

    /// Description: Helper function returning `ReadyState` and `Map` of scheduler, each in a MutexGuard
    /// switches Thread on fail, loops back after
    fn get_ready_state_and_join_map(&self) -> (MutexGuard<'_, ReadyState>, MutexGuard<'_, Map<usize, Vec<Arc<Thread>>>>) {
        loop {
            let ready_state = self.get_ready_state();
            if let Some(join_map) = self.join_map.try_lock() {
                return (ready_state, join_map);
            } else {
                self.switch_thread_no_interrupt();
            }
        }
    }

    /// For ps command - get all processes & threads
    pub fn get_status(&self, buffer: &mut [u8]) -> Result<usize, Errno> {
        let mut out = String::new();

        // Current
        let cur = self.current_thread();
        let _ = writeln!(out, "PID: {}, TID: {}, State: {:?}, Name: {}", cur.process().id(), cur.id(), ThreadState::Running, cur.process().name());

        // Ready Queue
        let state = self.get_ready_state();
        for thread in state.ready_queue.iter() {
            let _ = writeln!(out, "PID: {}, TID: {}, State: {:?}, Name: {}", thread.process().id(), thread.id(), thread.state(), thread.process().name());
        }

        // Sleep List
        let sleep_list = self.sleep_list.lock();
        for entry in sleep_list.iter() {
            // You used thread.0 in dump(), so keep that shape
            let t = &entry.0;
            let _ = writeln!(out, "PID: {}, TID: {}, State: {:?}, Name: {}", t.process().id(), t.id(), t.state(), t.process().name());
        }
        drop(sleep_list);

        // Block list
        let block_list = self.blocked_list.lock();
        for thread in block_list.iter() {
            let _ = writeln!(out, "PID: {}, TID: {}, State: {:?}, Name: {}", thread.process().id(), thread.id(), thread.state(), thread.process().name());
        }
        drop(block_list);

        // Copy to caller buffer (truncate if needed)
        let bytes = out.as_bytes();
        let len = core::cmp::min(bytes.len(), buffer.len());
        buffer[..len].copy_from_slice(&bytes[..len]);
        Ok(len)
    }

    /// Handle a command received via inbox, using the already-held ready_state lock (`state`).
    fn handle_inbox_cmd(&self, cmd: MessageCmd, state: &mut ReadyState) {
        match cmd {
            // Wake local join-waiters and add to readyQueue
            MessageCmd::JoinTargetExited { tid } => {
                let mut join_map = self.join_map.lock();

                if let Some(join_list) = join_map.get_mut(&tid) {
                    for waiter in join_list.drain(..) {
                        waiter.set_state(ThreadState::Running);
                        state.ready_queue.push_front(waiter);
                        inc_rq_len();
                    }
                    join_map.remove(&tid);
                }
            }
            // If the thread is locally blocked, requeue it.
            MessageCmd::Deblock { pid, tid } => {
                let mut blocked_list = self.blocked_list.lock();
                if is_thread_alive(tid) == false { return; }

                if let Some(pos) = blocked_list
                    .iter().position(|t| t.id() == tid && t.process().id() == pid)
                {
                    let thread = blocked_list.remove(pos);
                    thread.set_state(ThreadState::Running);
                    state.ready_queue.push_front(thread);
                    inc_rq_len();
                }
            }
            // if you have this thread, kill it
            MessageCmd::Kill { tid } => {
                if is_thread_alive(tid) == false { return; }
                let thread = self.current_thread();
                if thread.id() == tid { //cant kill itself, reschedule for other thread
                    let _ = schedule_on(current_core_id() as usize, MessageItem::Cmd(MessageCmd::Kill { tid }));
                    let _ = schedule_on_all_others(MessageItem::Cmd(MessageCmd::Kill { tid }));    //if target migrates until then
                    return;
                }
                self.kill_locally(tid, state);
            }
        }
    }

    
    /// Voluntarily yield the CPU to another runnable thread.
    ///
    /// Requirements / assumptions:
    /// - Must be called when it is safe to switch (no stack locks, tss not locked).
    /// - Does not change Parking/Blocked semantics; caller should set state beforehand if needed.
    pub fn yield_now(&self) {
        let mut state = self.get_ready_state();

        if !state.initialized {
            return;
        }

        // Current thread
        let current = self.current_thread();

        // Same restrictions as your timer-driven switching path
        if current.stacks_locked() || tss().is_locked() {
            return;
        }

        // If there is nobody else runnable, don't bother.
        // (Note: ready_queue does NOT include the current thread yet.)
        if state.ready_queue.is_empty() {
            return;
        }

        // Requeue current as Ready
        current.set_state(ThreadState::Ready);
        state.ready_queue.push_front(Arc::clone(&current));

        // Pick next
        let next = match state.ready_queue.pop_back() {
            Some(t) => t,
            None => {
                // Shouldn't happen because we checked !empty, but be safe
                current.set_state(ThreadState::Running);
                return;
            }
        };

        next.set_state(ThreadState::Running);
        // If we ended up picking ourselves (possible if only ourselves was in queue),
        // return.
        if next.id() == current.id() {
            return;
        }

        // Switch to next
        let current_ptr = core::ptr::from_ref(current.as_ref());
        let next_ptr = core::ptr::from_ref(next.as_ref());
        self.current_thread.set(Some(next));


        // ready_state is unlocked in your asm trampoline via unlock_scheduler()
        // (Thread::switch ultimately calls unlock_scheduler after switch)
        drop(current);

        unsafe {
            Thread::switch(current_ptr, next_ptr);
        }
    }
}


//// Multicore support  ////

/// Helper struct to store shared public information of a core
///     rq_len: approximate runqueue length (owner updates, others read)
///     resched_flag: indicates whether another core requested a reschedule
///     tx: sender for other cores to send messages to this one
///     apic_id: stored at initialization, used to translate cpu_id to apic_id for IPI's
#[repr(align(64))]
pub struct PerCpuRef {
    rq_len: AtomicU32,
    resched_flag: AtomicBool,
    tx: Sender<Option<MessageItem>>,    // producers (remote cores)
    apic_id: AtomicU32,
}
unsafe impl Sync for PerCpuRef {}
impl PerCpuRef { pub fn new(tx: Sender<Option<MessageItem>>) -> Self {
    Self { rq_len: AtomicU32::new(0), resched_flag: AtomicBool::new(false),
        tx, apic_id: AtomicU32::new(0) } } }


/// Wrapper enum to store either a runnable thread or a small cross-core command.
/// (Should be kept "small" since it lives in per-core inboxes and is drained in scheduler paths)
#[derive(Clone)]
pub enum MessageItem {
    Thread(Arc<Thread>),
    Cmd(MessageCmd),
}

impl MessageItem {
    #[inline]
    pub fn new_thread(thread: Arc<Thread>) -> Self {
        MessageItem::Thread(thread)
    }
    #[inline]
    pub fn new_cmd(cmd: MessageCmd) -> Self {
        MessageItem::Cmd(cmd)
    }
}


/// Commands that a core can request another core to perform locally
#[derive(Clone)]
pub enum MessageCmd {
    /// "Thread with `tid` exited somewhere; if you have join-waiters for it locally, wake them."
    JoinTargetExited { tid: usize },

    /// "If you have this thread in your local blocked list, wake it."
    Deblock { pid: Uuid, tid: usize },

    /// "If you have this thread, kill it."
    Kill { tid: usize },
}

/// Called only once by each owner core during startup to set the PER_CPU_SCHED apic_id
pub fn set_inbox_apic_id(id: usize) {
    let curr_id = id;
    let apic_id = get_apic_id();
    info!("Setting inbox{} apic_id to {}", curr_id, apic_id);
    per_cpu_ref(curr_id).apic_id.store(apic_id, Ordering::Release);
}

/// Returns the apic_id of the core with the given id
pub fn per_cpu_apic_id(cpu_id: usize) -> u32 {
    per_cpu_ref(cpu_id).apic_id.load(Ordering::Acquire)
}

/// Returns a reference to the PerCpuSched struct of the current core
#[inline]
pub fn per_cpu_ref_curr() -> &'static PerCpuRef {
    per_cpu_ref(current_core_id() as usize)
}

/// Returns a reference to the sender out of the PerCpuSched struct of the core with the given id
#[inline]
pub fn per_cpu_sender(id: usize) -> &'static Sender<Option<MessageItem>> {
    &per_cpu_ref(id).tx
}

/// Schedules a thread (wrapped in a MessageItem) on a remote core with the target_id
/// Sends a reschedule IPI to wake that core up, if it was idle.
pub fn schedule_on(target_id: usize, item: MessageItem) -> Result<(), MessageItem> {
    let pc = per_cpu_ref(target_id);
    match pc.tx.try_send(Some(item)) {
        Ok(()) => {
            send_reschedule_ipi(target_id);
            Ok(())
        }
        Err(e) => Err(e.into_inner().unwrap()), // unwrap: we sent Some(_)
    }
}

/// Schedules a cmd (wrapped in a MessageItem) on ALL remote cores
/// Does not send reschedule IPI's since only one core needs to actually do something
pub fn schedule_on_all_others(item: MessageItem) {
    let own_id = current_core_id() as usize;
    for i in 0..ACTIVE_CPUS.load(Ordering::Acquire) as usize {
        if i != own_id {
            let pc = per_cpu_ref(i);
            pc.tx.try_send(Some(item.clone())).expect("Failed to send message to remote core!");
        }
    }
}

/// drains the inbox from the cls into the ready queue; 10 items max per call
/// automatically calls inc_rq_len()
pub fn drain_inbox_into_ready(max: usize, state: &mut ReadyState) {
    let mut drained_threads = 0usize;
    for _ in 0..max {
        match cls().try_recv() {
            Ok(Some(item)) => match item {
                MessageItem::Thread(thread) => {

                    state.ready_queue.push_front(thread);
                    inc_rq_len();
                    drained_threads += 1;
                }
                MessageItem::Cmd(cmd) => {
                    scheduler().handle_inbox_cmd(cmd, state);
                }
            },
            Ok(None) => {
                log::error!("cpu{}: inbox returned None (unexpected)", current_core_id());
                break;
            }
            Err(_) => break,
        }
    }
    if drained_threads > 0 {
        clear_resched_flag();
        debug!("cpu{}: drained {} thread(s) into ready_queue", current_core_id(), drained_threads);
    }
}

/// Sends a Reschedule IPI to wake the core with the given id up, if it was idle.
fn send_reschedule_ipi(target_id: usize) {
    apic().send_reschedule(per_cpu_apic_id(target_id))
}
/// Owner function to set the reschedule flag of the current core.
pub fn set_resched_flag() {
    per_cpu_ref(current_core_id() as usize).resched_flag.store(true, Ordering::Release);
}
/// Owner function to clear the reschedule flag of the current core.
pub fn clear_resched_flag() {
    per_cpu_ref(current_core_id() as usize).resched_flag.store(false, Ordering::Release);
}
/// Owner function to read the reschedule flag of the current core.
pub fn read_resched_flag() -> bool {
    per_cpu_ref(current_core_id() as usize).resched_flag.load(Ordering::Acquire)
}
/// Owner function to increase the runqueue length of the current core.
pub fn inc_rq_len() {
    let id = current_core_id() as usize;
    per_cpu_ref(id).rq_len.fetch_add(1, Ordering::Relaxed);
}
/// Owner function to decrease the runqueue length of the current core.
pub fn dec_rq_len() {
    let id = current_core_id() as usize;
    per_cpu_ref(id).rq_len.fetch_sub(1, Ordering::Release); // Release for stronger publication before donating
}
/// Owner function to read the runqueue length of the current core.
pub fn read_rq_len() -> u32 {
    per_cpu_ref(current_core_id() as usize).rq_len.load(Ordering::Acquire)
}
/// Remote Reader function to read the runqueue length of the given Core
pub fn read_rq_len_remote(target_id: usize) -> u32 {
    per_cpu_ref(target_id).rq_len.load(Ordering::Acquire)
}

/// Idle_thread thread that checks for available Threads on other Cores and then
/// halts the cpu until it gets woken up by interrupts. (Other Cpus need to send first)
extern "sysv64" fn idle_thread () -> () {   //should never return but new_kernel_thread requires it
    loop {
        scheduler().look_for_overloaded_core();
        unsafe {
            asm!("hlt");
        }
    }
}