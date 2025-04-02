use crate::capabilities::capability::{Capability, CapabilityFlags};

pub struct Syscall {
    number: usize,
    function: *const (),
}

impl Syscall {
    pub fn new(number: usize, function: *const ()) -> Self {
        Self { number, function }
    }

    pub fn function_pointer(&self) -> *const () {
        self.function
    }

    pub fn number(&self) -> usize {self.number}
}


unsafe impl Send for Syscall {}
unsafe impl Sync for Syscall {}
