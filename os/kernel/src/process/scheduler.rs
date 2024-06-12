use crate::process::thread::Thread;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use smallmap::Map;
use spin::{Mutex, MutexGuard};
use crate::{allocator, apic, scheduler, timer, tss};

static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub fn next_thread_id() -> usize {
    THREAD_ID_COUNTER.fetch_add(1, Relaxed)
}

struct ReadyState {
    initialized: bool,
    current_thread: Option<Rc<Thread>>,
    ready_queue: VecDeque<Rc<Thread>>
}

impl ReadyState {
    pub fn new() -> Self {
        Self { initialized: false, current_thread: None, ready_queue: VecDeque::new() }
    }
}

pub struct Scheduler {
    ready_state: Mutex<ReadyState>,
    sleep_list: Mutex<Vec<(Rc<Thread>, usize)>>,
    join_map: Mutex<Map<usize, Vec<Rc<Thread>>>>
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

/// Called from assembly code, after the thread has been switched
#[no_mangle]
pub unsafe extern "C" fn unlock_scheduler() {
    unsafe { scheduler().ready_state.force_unlock(); }
}

impl Scheduler {
    pub fn new() -> Self {
        Self { ready_state: Mutex::new(ReadyState::new()), sleep_list: Mutex::new(Vec::new()), join_map: Mutex::new(Map::new()) }
    }

    pub fn set_init(&self) {
        self.get_ready_state().initialized = true;
    }

    pub fn active_thread_ids(&self) -> Vec<usize> {
        let state = self.get_ready_state();
        let sleep_list = self.sleep_list.lock();

        state.ready_queue.iter().map(|thread| thread.id()).collect::<Vec<usize>>()
            .into_iter().chain(sleep_list.iter().map(|entry| entry.0.id())).collect()
    }

    pub fn current_thread(&self) -> Rc<Thread> {
        let state = self.get_ready_state();
        return Scheduler::current(&state);
    }

    pub fn thread(&self, thread_id: usize) -> Option<Rc<Thread>> {
        self.ready_state.lock()
            .ready_queue.iter().find(|thread| {
            thread.id() == thread_id
        }).cloned()
    }

    pub fn start(&self) {
        let mut state = self.get_ready_state();
        state.current_thread = state.ready_queue.pop_back();

        unsafe { Thread::start_first(state.current_thread.as_ref().expect("Scheduler: Failed to dequeue first thread!").as_ref()); }
    }

    pub fn ready(&self, thread: Rc<Thread>) {
        let id = thread.id();
        let mut join_map;
        let mut state;

        // If we get the lock on 'self.state' but not on 'self.join_map' the system hangs.
        // The scheduler is not able to switch threads anymore, because of 'self.state' is locked,
        // and we will never be able to get the lock on 'self.join_map'.
        // To solve this, we need to release the lock on 'self.state' in case we do not get
        // the lock on 'self.join_map' and let the scheduler switch threads until we get both locks.
        loop {
            let state_mutex = self.get_ready_state();
            let join_map_option = self.join_map.try_lock();

            if join_map_option.is_some() {
                state = state_mutex;
                join_map = join_map_option.unwrap();
                break;
            } else {
                self.switch_thread_no_interrupt();
            }
        }

        state.ready_queue.push_front(thread);
        join_map.insert(id, Vec::new());
    }

    pub fn sleep(&self, ms: usize) {
        let mut state = self.get_ready_state();
        let thread = Scheduler::current(&state);
        let wakeup_time = timer().read().systime_ms() + ms;

        { // Execute in own block, so that the lock is released automatically (block() does not return)
            let mut sleep_list = self.sleep_list.lock();
            sleep_list.push((thread, wakeup_time));
        }

        self.block(&mut state);
    }

    fn switch_thread(&self, interrupt: bool) {
        if let Some(mut state) = self.ready_state.try_lock() {
            if !state.initialized {
                return;
            }

            if let Some(mut sleep_list) = self.sleep_list.try_lock() {
                Scheduler::check_sleep_list(&mut state, &mut sleep_list);
            }

            let current = Scheduler::current(&state);
            let next = match state.ready_queue.pop_back() {
                Some(thread) => thread,
                None => return,
            };

            // Current thread is initializing itself and may not be interrupted
            if current.stacks_locked() || tss().is_locked() {
                return;
            }

            let current_ptr = ptr::from_ref(current.as_ref());
            let next_ptr = ptr::from_ref(next.as_ref());

            state.current_thread = Some(next);
            state.ready_queue.push_front(current);

            if interrupt {
                apic().end_of_interrupt();
            }

            unsafe { Thread::switch(current_ptr, next_ptr); }
        }
    }

    pub fn switch_thread_no_interrupt(&self) {
        self.switch_thread(false);
    }

    pub fn switch_thread_from_interrupt(&self) {
        self.switch_thread(true);
    }

    pub fn join(&self, thread_id: usize) {
        let mut state = self.get_ready_state();
        let thread = Scheduler::current(&state);

        { // Execute in own block, so that the lock is released automatically (block() does not return)
            let mut join_map = self.join_map.lock();
            let join_list = join_map.get_mut(&thread_id);
            if join_list.is_some() {
                join_list.unwrap().push(thread);
            } else {
                // Joining on a non-existent thread has no effect (i.e. the thread has already finished running)
                return;
            }
        }

        self.block(&mut state);
    }

    pub fn exit(&self) {
        let mut ready_state;
        let current;

        { // Execute in own block, so that join_map is released automatically (block() does not return)
            let state = self.get_ready_state_and_join_map();
            ready_state = state.0;
            let mut join_map = state.1;

            current = Scheduler::current(&ready_state);
            let join_list = join_map.get_mut(&current.id()).expect(format!("Scheduler: Missing join_map entry for thread id {}!", current.id()).as_str());

            for thread in join_list {
                ready_state.ready_queue.push_front(Rc::clone(thread));
            }

            join_map.remove(&current.id());
        }

        drop(current); // Decrease Rc manually, because block() does not return
        self.block(&mut ready_state);
    }

    pub fn kill(&self, thread_id: usize) {
        { // Check if current thread tries to kill itself (illegal)
            let ready_state = self.get_ready_state();
            let current = Scheduler::current(&ready_state);

            if current.id() == thread_id {
                panic!("Scheduler: A thread cannot kill itself!");
            }
        }

        let state = self.get_ready_state_and_join_map();
        let mut ready_state = state.0;
        let mut join_map = state.1;

        let join_list = join_map.get_mut(&thread_id).expect(format!("Scheduler: Missing join_map entry for thread id {}!", thread_id).as_str());

        for thread in join_list {
            ready_state.ready_queue.push_front(Rc::clone(thread));
        }

        join_map.remove(&thread_id);
        ready_state.ready_queue.retain(|thread| thread.id() != thread_id);
    }

    fn block(&self, state: &mut ReadyState) {
        let mut next_thread = state.ready_queue.pop_back();

        { // Execute in own block, so that the lock is released automatically (block() does not return)
            let mut sleep_list = self.sleep_list.lock();
            while next_thread.is_none() {
                Scheduler::check_sleep_list(state, &mut sleep_list);
                next_thread = state.ready_queue.pop_back();
            }
        }

        let current = Scheduler::current(&state);
        let next = next_thread.unwrap();

        // Thread has enqueued itself into sleep list and waited so long, that it dequeued itself in the meantime
        if current.id() == next.id() {
            return;
        }

        let current_ptr = ptr::from_ref(current.as_ref());
        let next_ptr = ptr::from_ref(next.as_ref());

        state.current_thread = Some(next);
        drop(current); // Decrease Rc manually, because Thread::switch does not return

        unsafe { Thread::switch(current_ptr, next_ptr); }
    }

    fn current(state: &ReadyState) -> Rc<Thread> {
        return Rc::clone(state.current_thread.as_ref().expect("Scheduler: Trying to access current thread before initialization!"));
    }

    fn check_sleep_list(state: &mut ReadyState, sleep_list: &mut Vec<(Rc<Thread>, usize)>) {
        if let Some(timer) = timer().try_read() {
            let time = timer.systime_ms();

            sleep_list.retain(|entry| {
                if time >= entry.1 {
                    state.ready_queue.push_front(Rc::clone(&entry.0));
                    return false;
                }

                return true;
            });
        }
    }

    fn get_ready_state(&self) -> MutexGuard<ReadyState> {
        let state;

        // We need to make sure, that both the kernel memory manager and the ready queue are currently not locked.
        // Otherwise, a deadlock may occur: Since we are holding the ready queue lock,
        // the scheduler won't switch threads anymore, and none of the locks will ever be released
        loop {
            let state_tmp = self.ready_state.lock();
            if allocator().is_locked() {
                continue;
            }
            
            state = state_tmp;
            break;
        }

        return state;
    }

    fn get_ready_state_and_join_map(&self) -> (MutexGuard<ReadyState>, MutexGuard<Map<usize, Vec<Rc<Thread>>>>) {
        loop {
            let ready_state = self.get_ready_state();
            let join_map = self.join_map.try_lock();

            if join_map.is_some() {
                return (ready_state, join_map.unwrap());
            } else {
                self.switch_thread_no_interrupt();
            }
        }
    }
}
