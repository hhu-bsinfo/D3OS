use alloc::boxed::Box;
use core::ptr;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use nolock::queues::{DequeueError, mpsc};
use nolock::queues::mpsc::jiffy::{Receiver, Sender};
use crate::kernel::thread::thread::{start_first_thread, switch_thread, Thread};

static THREAD_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

pub fn next_thread_id() -> usize {
    THREAD_ID_COUNTER.fetch_add(1, Relaxed)
}

pub struct Scheduler {
    current_thread: *mut Thread,
    ready_queue: Option<(Receiver<Box<Thread>>, Sender<Box<Thread>>)>,
    initialized: bool
}

impl Scheduler {

    pub const fn new() -> Self {
        Self { current_thread: ptr::null_mut(), ready_queue: None, initialized: false }
    }

    pub fn init_queue(&mut self) {
        self.ready_queue = Some(mpsc::jiffy::queue());
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
        if self.ready_queue.is_none() {
            panic!("Scheduler: Trying to start scheduler before queue has been initialized!");
        }

        let thread = match self.ready_queue.as_mut().unwrap().0.try_dequeue() {
            Ok(thread) => thread,
            Err(error) => panic!("Scheduler: Failed to dequeue first thread (Error: {:?})", error)
        };

        unsafe {
            let current = Box::into_raw(thread);
            self.current_thread = current;
            start_first_thread(current);
        }
    }

    pub fn ready(&mut self, thread: Box<Thread>) {
        if self.ready_queue.is_none() {
            panic!("Scheduler: Trying to enqueue thread before queue has been initialized!");
        }

        if self.ready_queue.as_mut().unwrap().1.enqueue(thread).is_err() {
            panic!("Scheduler: Failed to enqueue thread!");
        }
    }

    pub fn switch_thread(&mut self) {
        if !self.initialized {
            return;
        }

        let queue = self.ready_queue.as_mut().unwrap();
        let current = self.current_thread;
        let next = match queue.0.try_dequeue() {
            Ok(thread) => thread,
            Err(error) => {
                if error == DequeueError::Empty {
                    return;
                }

                panic!("Scheduler: Failed to dequeue next thread (Error: {:?})", error)
            }
        };

        unsafe { self.ready(Box::from_raw(current)) }
        self.current_thread = Box::into_raw(next);

        unsafe { switch_thread(current, self.current_thread); }
    }
}