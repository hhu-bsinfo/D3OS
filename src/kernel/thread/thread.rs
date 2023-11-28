use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ptr;
use lazy_static::lazy_static;
use crate::kernel;
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

pub fn start_first_thread(thread: &Thread) {
    unsafe { thread_start(thread.old_rsp0); }
}

pub fn switch_thread(current: &Thread, next: &Thread) {
    unsafe { thread_switch(ptr::from_ref(&current.old_rsp0) as *mut u64, next.old_rsp0); }
}

impl Thread {
    pub fn new(entry: Box<dyn FnMut()>) -> Rc<Thread> {
        let mut thread = Thread{ id: scheduler::next_thread_id(), stack: Vec::with_capacity(STACK_SIZE / 8), old_rsp0: 0, entry };
        let stack = &mut thread.stack;

        if stack.capacity() % 8 != 0 {
            panic!("Thread: Stack size must be a multiple of 8!");
        }

        for _ in 0 .. stack.capacity() - INIT_STACK_ENTRIES {
            stack.push(0);
        }

        thread.prepare_stack();
        return Rc::new(thread);
    }

    pub fn kickoff() {
        let scheduler = kernel::get_thread_service().get_scheduler();
        scheduler.set_init();

        unsafe {
            let thread_ptr = ptr::from_ref(scheduler.get_current_thread().as_ref()) as *mut Thread;
            ((*thread_ptr).entry)();
        }

        scheduler.exit();
    }

    #[allow(dead_code)]
    pub fn join(&self) {
        let scheduler = kernel::get_thread_service().get_scheduler();
        scheduler.join(self.id);
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