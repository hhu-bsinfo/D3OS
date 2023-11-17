use crate::kernel::Service;
use crate::kernel::thread::scheduler::Scheduler;

pub struct ThreadService {
    scheduler: Scheduler
}

impl Service for ThreadService {}

impl ThreadService {
    pub const fn new() -> Self {
        Self { scheduler: Scheduler::new() }
    }

    pub fn get_scheduler(&mut self) -> &mut Scheduler {
        return &mut self.scheduler;
    }
}