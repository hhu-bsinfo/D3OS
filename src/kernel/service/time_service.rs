use core::hint::spin_loop;
use crate::device::pit::Pit;
use crate::kernel::Service;

pub struct TimeService {
    timer: Pit,
    systime_ns: usize
}

impl Service for TimeService {}

impl TimeService {
    pub const fn new() -> Self {
        Self { timer: Pit::new(), systime_ns: 0 }
    }

    pub fn init(&mut self) {
        self.timer.set_int_rate(1);
        self.timer.plugin();
    }

    pub fn inc_systime(&mut self, ns: usize) {
        self.systime_ns += ns;
    }

    pub fn get_systime_ms(&self) -> usize {
        return self.systime_ns / 1000000;
    }

    pub fn wait(&self, ms: usize) {
        let end_time = self.get_systime_ms() + ms;
        while self.get_systime_ms() < end_time {
            spin_loop();
        }
    }
}