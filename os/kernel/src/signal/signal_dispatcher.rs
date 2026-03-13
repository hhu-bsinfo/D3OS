use signal::signal_handler::SignalHandler;
use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;
use core::panic;
use core::convert::TryFrom;
use core::result::Result;
use core::result::Result::{Ok, Err};
use core::marker::{Sync, Send};
use core::option::Option::Some;
use signal::signal_vector::{SignalVector, MAX_VECTORS};
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::VirtAddr;
use terminal::println;
use crate::scheduler;

pub struct SignalDispatcher {
    int_vectors: Vec<Mutex<u64>>,
}

unsafe impl Send for SignalDispatcher {}
unsafe impl Sync for SignalDispatcher {}

pub fn handle_signal() {
	println!("Handling signal...");
}

impl SignalDispatcher {
    pub fn new() -> Self {
        let mut int_vectors = Vec::<Mutex<u64>>::new();
        for _ in 0..MAX_VECTORS {
            int_vectors.push(Mutex::new(0));
        }

        Self { int_vectors }
    }

    pub fn assign(&self, vector: SignalVector, handler: u64) {
        match self.int_vectors.get(vector as usize) {
            Some(vec) => *vec.lock() = handler,
            None => panic!("Assigning signal handler to illegal vector number {}!", vector as u8)
        }
    }

	pub fn dispatch(&self, signal: SignalVector, frame: &mut InterruptStackFrame) -> Result<(), ()> {
		//println!("Dispatching signal");
		let handle_signal;
		match self.get(signal) {
			Some(address) => handle_signal = address,
			None => return Err(()),
		}
		
		unsafe {
			frame.as_mut().update(|frame| {
				let stack_pointer: *mut u64 = frame.stack_pointer.as_mut_ptr();
				
				stack_pointer.write(frame.instruction_pointer.as_u64());
				frame.stack_pointer -= 8;
				frame.instruction_pointer = VirtAddr::new(handle_signal as u64);
			});
		}
		//println!("Updated rip, frame at {:p}: {:?}", &frame, frame);
		scheduler().current_thread().set_signal_pending(SignalVector::SIGSEGV);
		// When signals aren't handled immediately, we need to switch threads after setting the pending signal
		//scheduler().switch_thread_from_interrupt();
		return Ok(());
	}
    
    pub fn get(&self, vector: SignalVector) -> Option<u64> {
        match self.int_vectors.get(vector as usize) {
            Some(vec) => Some(*vec.lock()), // Todo do we need to free the lock?
            None => None
        }
    }
}
