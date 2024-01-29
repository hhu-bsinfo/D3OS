use crate::process::thread::Thread;
use alloc::collections::VecDeque;
use alloc::format;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use smallmap::Map;
use spin::Mutex;
use crate::{apic, timer};

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

impl Scheduler {
    pub fn new() -> Self {
        Self { state: Mutex::new(ReadyState::new()), sleep_list: Mutex::new(Vec::new()), join_map: Mutex::new(Map::new()) }
    }

    pub fn set_init(&self) {
        self.state.lock().initialized = true;
    }

    pub fn current_thread(&self) -> Rc<Thread> {
        let state = self.state.lock();
        return Scheduler::current(&state);
    }

    pub fn start(&self) {
        let thread;

        {
            let mut state = self.state.lock();
            thread = state.ready_queue.pop_back().expect("Scheduler: Failed to dequeue first thread!");
            state.current_thread = Some(Rc::clone(&thread));
        }

        Thread::start_first(thread.as_ref());
    }

    pub fn ready(&self, thread: Rc<Thread>) {
        let id = thread.id();
        let mut state = self.state.lock();
        let mut join_map = self.join_map.lock();

        state.ready_queue.push_front(thread);
        join_map.insert(id, Vec::new());
    }

    pub fn sleep(&self, ms: usize) {
        {
            let wakeup_time = timer().read().systime_ms() + ms;
            let state = self.state.lock();
            let mut sleep_list = self.sleep_list.lock();

            let thread = Scheduler::current(&state);
            sleep_list.push((thread, wakeup_time));
        }

        self.block();
    }

    pub fn switch_thread(&self) {
        let current;
        let next;

        if let Some(mut state) = self.state.try_lock() {
            if !state.initialized {
                return;
            }

            if let Some(mut sleep_list) = self.sleep_list.try_lock() {
                Scheduler::check_sleep_list(&mut state, &mut sleep_list);
            }

            next = match state.ready_queue.pop_back() {
                Some(thread) => thread,
                None => return,
            };

            current = Scheduler::current(&state);
            state.current_thread = Some(Rc::clone(&next));

            state.ready_queue.push_front(Rc::clone(&current));
        } else {
            return;
        }

        apic().end_of_interrupt();
        Thread::switch(current.as_ref(), next.as_ref());
    }

    pub fn block(&self) {
        let current;
        let next;

        {
            let mut state = self.state.lock();
            let mut sleep_list = self.sleep_list.lock();
            let mut next_thread = state.ready_queue.pop_back();

            while next_thread.is_none() {
                Scheduler::check_sleep_list(&mut state, &mut sleep_list);
                next_thread = state.ready_queue.pop_back();
            }

            current = Scheduler::current(&state);
            next = next_thread.unwrap();
            state.current_thread = Some(Rc::clone(&next));

            // Thread has enqueued itself into sleep list and waited so long, that it dequeued itself in the meantime
            if current.id() == next.id() {
                return;
            }
        }

        Thread::switch(current.as_ref(), next.as_ref());
    }

    pub fn join(&self, thread_id: usize) {
        {
            let state = self.state.lock();
            let mut join_map = self.join_map.lock();

            let thread = Scheduler::current(&state);
            let join_list = join_map.get_mut(&thread_id).expect(format!("Scheduler: Missing join_map entry for thread id {}!", thread.id()).as_str());

            join_list.push(thread);
        }

        self.block();
    }

    pub fn exit(&self) {
        {
            let mut state = self.state.lock();
            let mut join_map = self.join_map.lock();

            let thread = Scheduler::current(&state);
            let join_list = join_map.get_mut(&thread.id()).expect(format!("Scheduler: Missing join_map entry for thread id {}!", thread.id()).as_str());

            for thread in join_list {
                state.ready_queue.push_front(Rc::clone(thread));
            }

            join_map.remove(&thread.id());

            if !thread.is_kernel_thread() {
                thread.process().exit();
            }
        }

        self.block();
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
