use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use crate::kernel;
use crate::kernel::interrupt_dispatcher::InterruptVector;
use crate::kernel::log::Logger;
use crate::kernel::thread::scheduler;

extern "C" {
    fn thread_start(old_rsp0: u64);
    fn thread_switch(current_rsp0: *mut u64, next_rsp0: u64);
}

lazy_static! {
static ref LOG: Logger = Logger::new("Scheduler");
}

const STACK_SIZE: usize = 1048576;
const INIT_STACK_ENTRIES: usize = 18;

pub struct Thread {
    id: usize,
    stack: Vec<u64>,
    old_rsp0: u64,
    entry: Box<dyn FnMut()>
}

pub unsafe fn start_first_thread(thread: *mut Thread) {
    thread_start((*thread).old_rsp0);
}

pub unsafe fn switch_thread(current: *mut Thread, next: *mut Thread) {
    thread_switch(&mut (*current).old_rsp0, (*next).old_rsp0)
}

impl Thread {
    pub fn new(entry: Box<dyn FnMut()>) -> Box<Thread> {
        let mut thread = Box::new(Thread{ id: scheduler::next_thread_id(), stack: Vec::with_capacity(STACK_SIZE / 8), old_rsp0: 0, entry });
        let stack = &mut thread.stack;

        if stack.capacity() % 8 != 0 {
            panic!("Thread: Stack size must be a multiple of 8!");
        }

        for _ in 0 .. stack.capacity() - INIT_STACK_ENTRIES {
            stack.push(0);
        }

        thread.prepare_stack();
        return thread;
    }

    pub unsafe fn kickoff() {
        let scheduler = kernel::get_thread_service().get_scheduler();
        let interrupt_service = kernel::get_interrupt_service();

        scheduler.set_init();
        interrupt_service.get_apic().send_eoi(InterruptVector::Pit);

        LOG.info(format!("Starting new thread with id [{}]", scheduler.get_current_thread().get_id()).as_str());

        ((*scheduler.get_current_thread()).entry)();
        loop {}
    }

    pub fn get_id(&self) -> usize {
        return self.id;
    }

    fn prepare_stack(&mut self) {
        self.stack.push(0); // rbp
        self.stack.push(0); // rdi
        self.stack.push(0); // rsi

        self.stack.push(0); // rdx
        self.stack.push(0); // rcx
        self.stack.push(0); // rbx
        self.stack.push(0); // rax

        self.stack.push(0); // r15
        self.stack.push(0); // r14
        self.stack.push(0); // r13
        self.stack.push(0); // r12
        self.stack.push(0); // r11
        self.stack.push(0); // r10
        self.stack.push(0); // r9
        self.stack.push(0); // r8

        self.stack.push(0x202); // rflags (Interrupts enabled)
        self.stack.push(Thread::kickoff as u64); // Address of 'kickoff()';
        self.stack.push(0x00DEAD00u64); // Dummy return address

        self.old_rsp0 = self.stack.as_mut_ptr() as u64 + ((self.stack.capacity() - INIT_STACK_ENTRIES) * 8) as u64;
    }
}