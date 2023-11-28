use alloc::collections::VecDeque;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use spin::{Mutex, MutexGuard};
use crate::kernel;
use crate::kernel::interrupt_dispatcher::InterruptVector;
use crate::kernel::thread::thread::{start_first_thread, switch_thread, Thread};

static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub fn next_thread_id() -> usize {
    THREAD_ID_COUNTER.fetch_add(1, Relaxed)
}

pub struct Scheduler {
    current_thread: Option<Rc<Thread>>,
    ready_queue: Mutex<VecDeque<Rc<Thread>>>,
    sleep_list: Mutex<Vec<(Rc<Thread>, usize)>>,
    initialized: bool
}

impl Scheduler {
    pub const fn new() -> Self {
        Self { current_thread: None, ready_queue: Mutex::new(VecDeque::new()), sleep_list: Mutex::new(Vec::new()), initialized: false }
    }

    pub fn set_init(&mut self) {
        self.initialized = true;
    }

    pub fn get_current_thread(&self) -> Rc<Thread> {
        match self.current_thread.as_ref() {
            Some(thread) => Rc::clone(thread),
            None => panic!("Scheduler: Trying to access current thread before initialization!")
        }
    }

    pub fn start(&mut self) {
        let thread;

        {
            let mut ready_queue = self.ready_queue.lock();
            thread = match ready_queue.pop_back() {
                Some(thread) => thread,
                None => panic!("Scheduler: Failed to dequeue first thread!")
            };

            self.current_thread = Some(Rc::clone(&thread));
        }

        start_first_thread(thread.as_ref());
    }

    pub fn ready(&mut self, thread: Rc<Thread>) {
        self.ready_queue.lock().push_front(thread);
    }

    pub fn sleep(&mut self, ms: usize) {
        {
            let wakeup_time = kernel::get_device_service().get_timer().get_systime_ms() + ms;
            let thread = self.get_current_thread();
            self.sleep_list.lock().push((thread, wakeup_time));
        }

        self.block();
    }

    pub fn switch_thread(&mut self) {
        if !self.initialized {
            return;
        }

        let current;
        let next;

        if let Some(mut ready_queue) = self.ready_queue.try_lock() {
            if let Some(mut sleep_list) = self.sleep_list.try_lock() {
                Scheduler::check_sleep_list(&mut ready_queue, &mut sleep_list);
            }

            next = match ready_queue.pop_back() {
                Some(thread) => thread,
                None => return
            };

            current = self.get_current_thread();
            self.current_thread = Some(Rc::clone(&next));

            ready_queue.push_front(Rc::clone(&current));
        } else {
            return;
        }

        kernel::get_interrupt_service().get_apic().send_eoi(InterruptVector::Pit);
        switch_thread(current.as_ref(), next.as_ref());
    }

    pub fn block(&mut self) {
        let current;
        let next;

        {
            let mut ready_queue = self.ready_queue.lock();
            let mut sleep_list = self.sleep_list.lock();
            let mut next_thread = ready_queue.pop_back();

            while next_thread.is_none() {
                Scheduler::check_sleep_list(&mut ready_queue, &mut sleep_list);
                next_thread = ready_queue.pop_back();
            }

            current = self.get_current_thread();
            next = next_thread.unwrap();
            self.current_thread = Some(Rc::clone(&next));

            // Thread has enqueued itself into sleep list and waited so long, that it dequeued itself in the meantime
            if current.get_id() == next.get_id() {
                return;
            }
        }

        switch_thread(current.as_ref(), next.as_ref());
    }

    fn check_sleep_list(ready_queue: &mut MutexGuard<VecDeque<Rc<Thread>>>, sleep_list: &mut MutexGuard<Vec<(Rc<Thread>, usize)>>) {
        let time = kernel::get_device_service().get_timer().get_systime_ms();

        sleep_list.retain(|entry| {
            if time >= entry.1 {
                ready_queue.push_front(Rc::clone(&entry.0));
                return false;
            }

            return true;
        });
    }
}