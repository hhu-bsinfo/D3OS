use alloc::rc::Rc;
use crate::kernel::Service;
use crate::kernel::thread::scheduler::Scheduler;
use crate::kernel::thread::thread::Thread;

pub struct ThreadService {
    scheduler: Option<Scheduler>
}

impl Service for ThreadService {}

impl ThreadService {
    pub const fn new() -> Self {
        Self { scheduler: None }
    }

    pub fn init(&mut self) {
        self.scheduler = Some(Scheduler::new());
    }

    pub fn start_scheduler(&mut self) {
        self.get_scheduler_mut().start();
    }

    pub fn ready_thread(&mut self, thread: Rc<Thread>) {
        self.get_scheduler_mut().ready(thread);
    }

    pub fn switch_thread(&mut self) {
        self.get_scheduler_mut().switch_thread();
    }

    pub fn sleep(&mut self, ms: usize) {
        self.get_scheduler_mut().sleep(ms);
    }

    pub fn set_scheduler_init(&mut self) {
        self.get_scheduler_mut().set_init();
    }

    pub fn get_current_thread(&self) -> Rc<Thread> {
        return self.get_scheduler_ref().get_current_thread();
    }

    pub fn exit_thread(&mut self) {
        self.get_scheduler_mut().exit();
    }

    pub fn join_thread(&mut self, thread_id: usize) {
        self.get_scheduler_mut().join(thread_id);
    }

    fn get_scheduler_ref(&self) -> &Scheduler {
        match self.scheduler.as_ref() {
            Some(scheduler) => scheduler,
            None => panic!("Thread Service: Trying to access scheduler before initialization!")
        }
    }

    fn get_scheduler_mut(&mut self) -> &mut Scheduler {
        match self.scheduler.as_mut() {
            Some(scheduler) => scheduler,
            None => panic!("Thread Service: Trying to access scheduler before initialization!")
        }
    }
}