use crate::process::scheduler;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::{mem, ptr};
use core::ops::Deref;
use goblin::elf64;
use goblin::elf::Elf;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::PrivilegeLevel::Ring3;
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::{memory, scheduler, tss};
use crate::process::process::{create_process, kernel_process, Process};

const STACK_SIZE_PAGES: usize = 16;
const USER_STACK_ADDRESS: usize = 0x400000000000;

pub struct Thread {
    id: usize,
    kernel_stack: Vec<u64>,
    user_stack: Vec<u64>,
    process: Arc<Process>,
    old_rsp0: VirtAddr,
    entry: Box<fn()>,
}

impl Thread {
    pub fn new_kernel_thread(entry: Box<fn()>) -> Rc<Thread> {
        let mut thread = Thread {
            id: scheduler::next_thread_id(),
            kernel_stack: Vec::with_capacity((STACK_SIZE_PAGES * PAGE_SIZE) / 8),
            user_stack: Vec::with_capacity(0),
            process: kernel_process().expect("Trying to create a kernel thread before process initialization!"),
            old_rsp0: VirtAddr::zero(),
            entry,
        };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    #[allow(dead_code)]
    pub fn new_user_thread(elf_buffer: &[u8]) -> Rc<Thread> {
        let process = create_process();
        let address_space = process.address_space();

        let elf = Elf::parse(elf_buffer).expect("Failed to parse application!");
        elf.program_headers.iter()
            .filter(|header| header.p_type == elf64::program_header::PT_LOAD)
            .for_each(|header| {
                let page_count = if header.p_memsz as usize % PAGE_SIZE == 0 { header.p_memsz as usize / PAGE_SIZE } else { (header.p_memsz as usize / PAGE_SIZE) + 1 };
                let frames = memory::physical::alloc(page_count);
                let virt_start = Page::from_start_address(VirtAddr::new(header.p_vaddr)).expect("ELF: Program section not page aligned!");
                let pages = PageRange { start: virt_start, end: virt_start + page_count as u64 };

                unsafe {
                    let code = elf_buffer.as_ptr().offset(header.p_offset as isize);
                    let target = frames.start.start_address().as_u64() as *mut u8;
                    target.copy_from(code, header.p_filesz as usize);
                    target.offset(header.p_filesz as isize).write_bytes(0, (header.p_memsz - header.p_filesz) as usize);
                }

                process.address_space().map_physical(frames, pages, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
            });

        let user_stack_start = Page::from_start_address(VirtAddr::new(USER_STACK_ADDRESS as u64)).unwrap();
        let user_stack = unsafe { Vec::from_raw_parts(USER_STACK_ADDRESS as *mut u64, 0, (STACK_SIZE_PAGES * PAGE_SIZE) / 8) };
        address_space.map(PageRange { start: user_stack_start, end: user_stack_start + STACK_SIZE_PAGES as u64 }, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);

        let mut thread = Thread {
            id: scheduler::next_thread_id(),
            kernel_stack: Vec::with_capacity((STACK_SIZE_PAGES * PAGE_SIZE) / 8),
            user_stack,
            process,
            old_rsp0: VirtAddr::zero(),
            entry: unsafe { Box::new(mem::transmute(elf.entry as *const ())) }
        };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    pub fn kickoff_kernel_thread() {
        let scheduler = scheduler();
        let thread = scheduler.current_thread();
        scheduler.set_init();

        unsafe {
            let thread_ptr = ptr::from_ref(thread.as_ref()) as *mut Thread;
            tss().lock().privilege_stack_table[0] = VirtAddr::new(thread.kernel_stack_addr() as u64);

            if thread.is_kernel_thread() {
                ((*thread_ptr).entry)();
            } else {
                (*thread_ptr).switch_to_user_mode();
            }
        }

        scheduler.exit();
    }

    pub fn start_first(thread: &Thread) {
        unsafe { thread_kernel_start(thread.old_rsp0.as_u64()) }
    }

    pub fn switch(current: &Thread, next: &Thread) {
        unsafe { thread_switch(ptr::from_ref(&current.old_rsp0) as *mut u64, next.old_rsp0.as_u64(), next.kernel_stack_addr() as u64, next.process().address_space().page_table_address().start_address().as_u64()); }
    }

    pub fn is_kernel_thread(&self) -> bool {
        return self.user_stack.capacity() == 0;
    }

    pub fn process(&self) -> Arc<Process> {
        return Arc::clone(&self.process);
    }

    #[allow(dead_code)]
    pub fn join(&self) {
        scheduler().join(self.id());
    }

    pub fn id(&self) -> usize {
        return self.id;
    }

    pub fn kernel_stack_addr(&self) -> *const u64 {
        unsafe { return self.kernel_stack.as_ptr().offset(((self.kernel_stack.capacity() - 1) * 8) as isize); }
    }

    fn prepare_kernel_stack(&mut self) {
        let stack_addr = self.kernel_stack.as_ptr() as u64;
        let capacity = self.kernel_stack.capacity();

        for _ in 0..self.kernel_stack.capacity() {
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

        self.old_rsp0 = VirtAddr::new(stack_addr + ((capacity - 18) * 8) as u64);
    }

    fn switch_to_user_mode(&mut self) {
        let kernel_stack_addr = self.kernel_stack.as_ptr() as u64;
        let user_stack_addr = self.user_stack.as_ptr() as u64;
        let capacity = self.kernel_stack.capacity();

        for _ in 0..self.user_stack.capacity() {
            self.user_stack.push(0);
        }

        self.kernel_stack[capacity - 7] = 0; // rdi
        self.kernel_stack[capacity - 6] = *self.entry.deref() as u64; // Address of 'kickoff_user_thread()'

        self.kernel_stack[capacity - 5] = SegmentSelector::new(4, Ring3).0 as u64; // cs = user code segment
        self.kernel_stack[capacity - 4] = 0x202; // rflags (Interrupts enabled)
        self.kernel_stack[capacity - 3] =
            user_stack_addr + (self.user_stack.capacity() - 1) as u64 * 8; // rsp for user stack
        self.kernel_stack[capacity - 2] = SegmentSelector::new(3, Ring3).0 as u64; // ss = user data segment

        self.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address

        self.old_rsp0 = VirtAddr::new(kernel_stack_addr + ((capacity - 7) * 8) as u64);

        unsafe { thread_user_start(self.old_rsp0.as_u64()); }
    }
}

#[naked]
unsafe extern "C" fn thread_kernel_start(old_rsp0: u64) {
    asm!(
    "mov rsp, rdi", // First parameter -> load 'old_rsp0'
    "pop rbp",
    "pop rdi", // 'old_rsp0' is here
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rbx",
    "pop rax",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "popf",

    "call unlock_scheduler",
    "ret",
    options(noreturn)
    );
}

#[naked]
unsafe extern "C" fn thread_user_start(old_rsp0: u64) {
    asm!(
    "mov rsp, rdi", // Load 'old_rsp' (first parameter)
    "pop rdi",
    "iretq", // Switch to user-mode
    options(noreturn)
    )
}

#[naked]
unsafe extern "C" fn thread_switch(current_rsp0: *mut u64, next_rsp0: u64, next_rsp0_end: u64, next_cr3: u64) {
    asm!(
    // Save registers of current thread
    "pushf",
    "push r8",
    "push r9",
    "push r10",
    "push r11",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "push rax",
    "push rbx",
    "push rcx",
    "push rdx",
    "push rsi",
    "push rdi",
    "push rbp",

    // Save stack pointer in 'current_rsp0' (first parameter)
    "mov [rdi], rsp",

    // Store rsi and rcx in r12 and r13, as they might be overwritten by the following function call
    "mov r12, rsi",
    "mov r13, rcx",

    // Set rsp0 of kernel stack in tss (third parameter 'next_rsp0_end')
    "mov rdi, rdx",
    "call tss_set_rsp0",

    // Restore rsi and rcx
    "mov rcx, r13",
    "mov rsi, r12",

    // Switch address space (fourth parameter 'next_cr3')
    "mov cr3, rcx",

    // Load registers of next thread by using 'next_rsp0' (second parameter)
    "mov rsp, rsi",
    "pop rbp",
    "pop rdi",
    "pop rsi",
    "pop rdx",
    "pop rcx",
    "pop rbx",
    "pop rax",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop r11",
    "pop r10",
    "pop r9",
    "pop r8",
    "popf",

    "call unlock_scheduler",
    "ret", // Return to next thread
    options(noreturn)
    )
}