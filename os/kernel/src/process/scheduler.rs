use crate::process::thread::Thread;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ptr;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use smallmap::Map;
use spin::Mutex;
use crate::{apic, scheduler, timer, tss};

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
    state: Mutex<ReadyState>,
    sleep_list: Mutex<Vec<(Rc<Thread>, usize)>>,
    join_map: Mutex<Map<usize, Vec<Rc<Thread>>>>
}

unsafe impl Send for Scheduler {}
unsafe impl Sync for Scheduler {}

/// Called from assembly code, after the thread has been switched
#[no_mangle]
pub unsafe extern "C" fn unlock_scheduler() {
    scheduler().state.force_unlock();
}

impl Scheduler {
    pub fn new() -> Self {
        Self { state: Mutex::new(ReadyState::new()), sleep_list: Mutex::new(Vec::new()), join_map: Mutex::new(Map::new()) }
    }

    pub fn set_init(&self) {
        self.state.lock().initialized = true;
    }

    pub fn active_thread_ids(&self) -> Vec<usize> {
        let state = self.state.lock();
        let sleep_list = self.sleep_list.lock();

        state.ready_queue.iter().map(|thread| thread.id()).collect::<Vec<usize>>()
            .into_iter().chain(sleep_list.iter().map(|entry| entry.0.id())).collect()
    }

    pub fn current_thread(&self) -> Rc<Thread> {
        let state = self.state.lock();
        return Scheduler::current(&state);
    }

    pub fn start(&self) {
        let mut state = self.state.lock();
        state.current_thread = state.ready_queue.pop_back();

        unsafe { Thread::start_first(state.current_thread.as_ref().expect("Scheduler: Failed to dequeue first thread!").as_ref()); }
    }

    pub fn ready(&self, thread: Rc<Thread>) {
        let id = thread.id();
        let mut state = self.state.lock();
        let mut join_map = self.join_map.lock();

        state.ready_queue.push_front(thread);
        join_map.insert(id, Vec::new());
    }

    pub fn sleep(&self, ms: usize) {
        let mut state = self.state.lock();
        let thread = Scheduler::current(&state);
        let wakeup_time = timer().read().systime_ms() + ms;

        { // Execute in own block, so that the lock is released automatically (block() does not return)
            let mut sleep_list = self.sleep_list.lock();
            sleep_list.push((thread, wakeup_time));
        }

        self.block(&mut state);
    }

    pub fn switch_thread(&self) {
        if let Some(mut state) = self.state.try_lock() {
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

            apic().end_of_interrupt();
            unsafe { Thread::switch(current_ptr, next_ptr); }
        }
    }

    pub fn join(&self, thread_id: usize) {
        let mut state = self.state.lock();
        let thread = Scheduler::current(&state);

        { // Execute in own block, so that the lock is released automatically (block() does not return)
            let mut join_map = self.join_map.lock();
            let join_list = join_map.get_mut(&thread_id).expect(format!("Scheduler: Missing join_map entry for thread id {}!", thread_id).as_str());
            join_list.push(thread);
        }

        self.block(&mut state);
    }

    pub fn exit(&self) {
        let mut state = self.state.lock();
        let current = Scheduler::current(&state);

        { // Execute in own block, so that join_map is released automatically when it is not needed anymore
            let mut join_map = self.join_map.lock();
            let join_list = join_map.get_mut(&current.id()).expect(format!("Scheduler: Missing join_map entry for thread id {}!", current.id()).as_str());

            for thread in join_list {
                state.ready_queue.push_front(Rc::clone(thread));
            }

            join_map.remove(&current.id());
        }

        if !current.is_kernel_thread() {
            current.process().exit();
        }

        drop(current); // Decrease Rc manually, because block() does not return
        self.block(&mut state);
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
}
