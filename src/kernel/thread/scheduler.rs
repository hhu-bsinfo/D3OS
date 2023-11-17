use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::ptr;
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
    current_thread: *mut Thread,
    ready_queue: Mutex<VecDeque<Box<Thread>>>,
    sleep_list: Mutex<Vec<(*mut Thread, usize)>>,
    initialized: bool
}

impl Scheduler {
    pub const fn new() -> Self {
        Self { current_thread: ptr::null_mut(), ready_queue: Mutex::new(VecDeque::new()), sleep_list: Mutex::new(Vec::new()), initialized: false }
    }

    pub fn set_init(&mut self) {
        self.initialized = true;
    }

    pub fn get_current_thread(&mut self) -> &mut Thread {
        match self.initialized {
            true => unsafe { self.current_thread.as_mut().unwrap_or_else(|| panic!("Scheduler: Failed to get current thread as ref!")) },
            false => panic!("Scheduler: Trying to access current thread before initialization!")
        }
    }

    pub fn start(&mut self) {
        let current;

        {
            let mut ready_queue = self.ready_queue.lock();
            let thread = match ready_queue.pop_back() {
                Some(thread) => thread,
                None => panic!("Scheduler: Failed to dequeue first thread!")
            };

            current = Box::into_raw(thread);
            self.current_thread = current;
        }

        if current.is_null() {
            panic!("Scheduler: Trying to start scheduler with no threads enqueued!");
        }

        unsafe { start_first_thread(current); }
    }

    pub fn ready(&mut self, thread: Box<Thread>) {
        self.ready_queue.lock().push_front(thread);
    }

    pub fn sleep(&mut self, ms: usize) {
        {
            let wakeup_time = kernel::get_device_service().get_timer().get_systime_ms() + ms;
            self.sleep_list.lock().push((self.current_thread, wakeup_time));
        }

        self.block();
    }

    pub fn switch_thread(&mut self) {
        if !self.initialized {
            return;
        }

        let mut current = ptr::null_mut();
        let mut next = ptr::null_mut();

        if let Some(mut ready_queue) = self.ready_queue.try_lock() {
            if let Some(mut sleep_list) = self.sleep_list.try_lock() {
                Scheduler::check_sleep_list(&mut ready_queue, &mut sleep_list);
            }

            let next_thread = match ready_queue.pop_back() {
                Some(thread) => thread,
                None => return
            };

            current = self.current_thread;
            next = Box::into_raw(next_thread);
            self.current_thread = next;

            unsafe { ready_queue.push_front(Box::from_raw(current)); }
        }

        if !current.is_null() && !next.is_null() {
            kernel::get_interrupt_service().get_apic().send_eoi(InterruptVector::Pit);
            unsafe { switch_thread(current, next); }
        }
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

            current = self.current_thread;
            next = Box::into_raw(next_thread.unwrap());
            self.current_thread = next;

            // Thread has enqueued itself into sleep list and waited so long, that it dequeued itself in the meantime
            if current == next {
                return;
            }
        }

        unsafe { switch_thread(current, next); }
    }

    fn check_sleep_list(ready_queue: &mut MutexGuard<VecDeque<Box<Thread>>>, sleep_list: &mut MutexGuard<Vec<(*mut Thread, usize)>>) {
        let time = kernel::get_device_service().get_timer().get_systime_ms();

        sleep_list.retain(|entry| {
            if time >= entry.1 {
                unsafe {
                    let thread = Box::from_raw(entry.0);
                    ready_queue.push_front(thread)
                }

                return false;
            }

            return true;
        });
    }
}