
use super::{ne2000::Ne2000};
use crate::interrupt::interrupt_handler::InterruptHandler;
use crate::memory::frames;
use alloc::sync::Arc;
// create Struct Interrupthandler, holds a reference to Ne2000
pub struct Ne2000InterruptHandler {
    device: Arc<Ne2000>,
}

// implement the InterruptHandler
// creates a new Instance of Ne2000InterruptHandler
impl Ne2000InterruptHandler {
    pub fn new(device: Arc<Ne2000>) -> Self {
        Self { device }
    }
}

impl InterruptHandler for Ne2000InterruptHandler {
    fn trigger(&self) {
        if self.device.registers {
            panic!("Interrupt status register is locked during interrupt!");
        }

        // Read interrupt status register (Each bit corresponds to an interrupt type or error)
        let mut status_reg = self.device.registers.;
        let status = Interrupt::from_bits_retain(unsafe { status_reg.read() });

        // Check error flags
        if status.contains(Interrupt::TRANSMIT_ERROR) {
            panic!("Transmit failed!");
        } else if status.contains(Interrupt::RECEIVE_ERROR) {
            panic!("Receive failed!");
        }

        // Writing the status register clears all bits.
        // According to the RTL8139 documentation, this is not necessary,
        // but QEMU and some hardware require clearing the interrupt status register.
        // Furthermore, this needs to be done before processing the received packet (https://wiki.osdev.org/RTL8139).
        unsafe {
            status_reg.write(status.bits());
        }

        // Handle transmit by freeing allocated buffers
        if status.contains(Interrupt::TRANSMIT_OK) && !frames::allocator_locked() {
            let mut queue = self.device.send_queue.0.lock();
            let mut buffer = queue.try_dequeue();
            while buffer.is_ok() {
                unsafe { frames::free(buffer.unwrap()) };
                buffer = queue.try_dequeue();
            }
        }

        // Handle receive interrupt by processing received packet
        if status.contains(Interrupt::RECEIVE_OK) {
            self.device.process_received_packet();
        }
    }
}
