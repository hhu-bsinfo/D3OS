use crate::process::scheduler::set_resched_flag;

pub trait InterruptHandler {
    fn trigger(&self);
}

// Simple reschedule IPI handler: set the per-CPU reschedule flag or poke the scheduler
pub struct ReschedIpiHandler;

impl InterruptHandler for ReschedIpiHandler {
    fn trigger(&self) {
        // flip the per-CPU flag:
        set_resched_flag();
    }
}
