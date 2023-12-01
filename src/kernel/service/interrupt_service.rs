use alloc::boxed::Box;
use crate::device::apic::Apic;
use crate::kernel;
use crate::kernel::interrupt_dispatcher::{InterruptDispatcher, InterruptVector};
use crate::kernel::isr::ISR;
use crate::kernel::Service;

pub struct InterruptService {
    apic: Apic,
    int_disp: InterruptDispatcher
}

impl Service for InterruptService {}

impl InterruptService {
    pub const fn new() -> Self {
        Self { apic: Apic::new(), int_disp: InterruptDispatcher::new() }
    }

    pub fn init(&mut self) {
        self.int_disp.init();
        self.apic.init();
    }

    pub fn allow_interrupt(&mut self, vector: InterruptVector) {
        self.apic.allow(vector);
    }

    pub fn end_of_interrupt(&mut self) {
        self.apic.send_eoi();
    }

    pub fn assign_handler(&mut self, vector: InterruptVector, isr: Box<dyn ISR>) {
        self.int_disp.assign(vector, isr);
    }

    pub fn dispatch_interrupt(&mut self, int_number: u32) {
        self.int_disp.dispatch(int_number)
    }
}

#[no_mangle]
pub extern "C" fn int_disp(int_number: u32) {
    kernel::get_interrupt_service().dispatch_interrupt(int_number);
}