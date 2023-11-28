use crate::kernel::Service;
use crate::kernel::thread::scheduler::Scheduler;

pub struct ThreadService {
    scheduler: Option<Scheduler>
}

impl Service for ThreadService {}

impl ThreadService {
    pub const fn new() -> Self {
        Self { scheduler: None }
    }

    pub fn initialize(&mut self) {
        self.scheduler = Some(Scheduler::new());
    }

    pub fn get_scheduler(&mut self) -> &mut Scheduler {
        match self.scheduler.as_mut() {
            Some(scheduler) => scheduler,
            None => panic!("Thread Service: Trying to access scheduler before initialization!")
        }
    }
}