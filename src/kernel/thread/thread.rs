use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::ptr;
use lazy_static::lazy_static;
use crate::kernel;
use crate::kernel::log::Logger;
use crate::kernel::syscall::user_api::thread_api;
use crate::kernel::thread::scheduler;

extern "C" {
    fn thread_kernel_start(old_rsp0: u64);
    fn thread_user_start(old_rsp0: u64);
    fn thread_switch(current_rsp0: *mut u64, next_rsp0: u64, next_rsp0_end: u64);
    fn tss_set_rsp0(old_rsp0: u64);
}

lazy_static! {
static ref LOG: Logger = Logger::new("Scheduler");
}

const STACK_SIZE: usize = 65536;

pub struct Thread {
    id: usize,
    kernel_stack: Vec<u64>,
    user_stack: Vec<u64>,
    old_rsp0: u64,
    entry: Box<dyn FnMut()>
}

impl Thread {
    pub fn new_kernel_thread(entry: Box<dyn FnMut()>) -> Rc<Thread> {
        let mut thread = Thread {
            id: scheduler::next_thread_id(),
            kernel_stack: Vec::with_capacity(STACK_SIZE / 8),
            user_stack: Vec::with_capacity(0),
            old_rsp0: 0,
            entry };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    #[allow(dead_code)]
    pub fn new_user_thread(entry: Box<dyn FnMut()>) -> Rc<Thread> {
        let mut thread = Thread {
            id: scheduler::next_thread_id(),
            kernel_stack: Vec::with_capacity(STACK_SIZE / 8),
            user_stack: Vec::with_capacity(STACK_SIZE / 8),
            old_rsp0: 0,
            entry };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    pub fn kickoff_kernel_thread() {
        let thread_service = kernel::get_thread_service();
        let thread = thread_service.get_current_thread();
        thread_service.set_scheduler_init();

        unsafe {
            let thread_ptr = ptr::from_ref(thread.as_ref()) as *mut Thread;
            tss_set_rsp0(thread.get_kernel_stack_addr() as u64);

            if thread.is_kernel_thread() {
                thread_service.set_scheduler_init();
                ((*thread_ptr).entry)();
            } else {
                (*thread_ptr).switch_to_user_mode();
            }
        }

        thread_service.exit_thread();
    }

    pub fn kickoff_user_thread() {
        let thread_service = kernel::get_thread_service();
        let thread = thread_service.get_current_thread();
        thread_service.set_scheduler_init();

        unsafe {
            let thread_ptr = ptr::from_ref(thread.as_ref()) as *mut Thread;
            ((*thread_ptr).entry)();
        }

        thread_api::usr_thread_exit();
    }

    pub fn start_first(thread: &Thread) {
        unsafe { thread_kernel_start(thread.old_rsp0); }
    }

    pub fn switch(current: &Thread, next: &Thread) {
        unsafe { thread_switch(ptr::from_ref(&current.old_rsp0) as *mut u64, next.old_rsp0, next.get_kernel_stack_addr() as u64); }
    }

    pub fn is_kernel_thread(&self) -> bool {
        return self.user_stack.capacity() == 0;
    }

    #[allow(dead_code)]
    pub fn join(&self) {
        kernel::get_thread_service().join_thread(self.get_id());
    }

    pub fn get_id(&self) -> usize {
        return self.id;
    }

    pub fn get_kernel_stack_addr(&self) -> *const u64 {
        unsafe { return self.kernel_stack.as_ptr().offset(((self.kernel_stack.capacity() - 1) * 8) as isize); }
    }

    fn prepare_kernel_stack(&mut self) {
        let stack_addr = self.kernel_stack.as_ptr() as u64;
        let capacity = self.kernel_stack.capacity();

        for _ in 0 .. self.kernel_stack.capacity() {
            self.kernel_stack.push(0);
        }
        
        self.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address
        self.kernel_stack[capacity - 2] = Thread::kickoff_kernel_thread as u64; // Address of 'kickoff_kernel_thread()';
        self.kernel_stack[capacity - 3] = 0x202; // rflags (Interrupts enabled)

        self.kernel_stack[capacity - 4] = 0; // r8
        self.kernel_stack[capacity - 5] = 0; // r9
        self.kernel_stack[capacity - 6] = 0; // r10
        self.kernel_stack[capacity - 7] = 0; // r11
        self.kernel_stack[capacity - 8] = 0; // r12
        self.kernel_stack[capacity - 9] = 0; // r13
        self.kernel_stack[capacity - 10] = 0; // r14
        self.kernel_stack[capacity - 11] = 0; // r15

        self.kernel_stack[capacity - 12] = 0; // rax
        self.kernel_stack[capacity - 13] = 0; // rbx
        self.kernel_stack[capacity - 14] = 0; // rcx
        self.kernel_stack[capacity - 15] = 0; // rdx

        self.kernel_stack[capacity - 16] = 0; // rsi
        self.kernel_stack[capacity - 17] = 0; // rdi
        self.kernel_stack[capacity - 18] = 0; // rbp

        self.old_rsp0 =  stack_addr + ((capacity - 18) * 8) as u64;
    }

    fn switch_to_user_mode(&mut self) {
        let kernel_stack_addr = self.kernel_stack.as_ptr() as u64;
        let user_stack_addr = self.user_stack.as_ptr() as u64;
        let capacity = self.kernel_stack.capacity();

        for _ in 0 .. self.user_stack.capacity() {
            self.user_stack.push(0);
        }

        self.kernel_stack[capacity - 7] = 0; // rdi
        self.kernel_stack[capacity - 6] = Thread::kickoff_user_thread as u64; // Address of 'kickoff_user_thread()'

        self.kernel_stack[capacity - 5] = 0x23; // cs = user code segment; 4. entry, rpl = 3
        self.kernel_stack[capacity - 4] = 0x202; // rflags (Interrupts enabled)
        self.kernel_stack[capacity - 3] = user_stack_addr + (self.user_stack.capacity() - 1) as u64 * 8; // rsp for user stack
        self.kernel_stack[capacity - 2] = 0x2b; // ss = user data segment; 5. entry, rpl = 3

        self.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address

        self.old_rsp0 =  kernel_stack_addr + ((capacity - 7) * 8) as u64;

        unsafe { thread_user_start(self.old_rsp0); }
    }
}