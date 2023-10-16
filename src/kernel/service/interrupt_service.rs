use crate::device::apic::Apic;
use crate::kernel::interrupt_dispatcher::InterruptDispatcher;
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

    pub fn get_apic(&mut self) -> &mut Apic {
        return &mut self.apic;
    }

    pub fn get_dispatcher(&mut self) -> &mut InterruptDispatcher {
        return &mut self.int_disp;
    }
}