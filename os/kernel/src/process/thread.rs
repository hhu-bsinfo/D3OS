use crate::process::scheduler;
use alloc::rc::Rc;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::{mem, ptr};
use goblin::elf64;
use goblin::elf::Elf;
use spin::Mutex;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::PrivilegeLevel::Ring3;
use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::{memory, process_manager, scheduler, tss};
use crate::memory::alloc::StackAllocator;
use crate::memory::r#virtual::{VirtualMemoryArea, VmaType};
use crate::process::process::Process;
use crate::syscall::syscall_dispatcher::CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX;

pub const MAIN_USER_STACK_START: usize = 0x400000000000;
pub const MAX_USER_STACK_SIZE: usize = 0x40000000;
const KERNEL_STACK_PAGES: usize = 64;

struct Stacks {
    kernel_stack: Vec<u64, StackAllocator>,
    user_stack: Vec<u64, StackAllocator>,
    old_rsp0: VirtAddr
}

pub struct Thread {
    id: usize,
    stacks: Mutex<Stacks>,
    process: Arc<Process>,
    entry: fn(),
    user_rip: VirtAddr
}

impl Stacks {
    const fn new(kernel_stack: Vec<u64, StackAllocator>, user_stack: Vec<u64, StackAllocator>) -> Self {
        Self { kernel_stack, user_stack, old_rsp0: VirtAddr::zero() }
    }
}

impl Thread {
    pub fn new_kernel_thread(entry: fn()) -> Rc<Thread> {
        let kernel_stack = Vec::<u64, StackAllocator>::with_capacity_in((KERNEL_STACK_PAGES * PAGE_SIZE) / 8, StackAllocator::new());
        let user_stack = Vec::with_capacity_in(0, StackAllocator::new()); // Dummy stack

        let thread = Thread {
            id: scheduler::next_thread_id(),
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process: process_manager().read().kernel_process().expect("Trying to create a kernel thread before process initialization!"),
            entry,
            user_rip: VirtAddr::zero()
        };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    pub fn load_application(elf_buffer: &[u8]) -> Rc<Thread> {
        let process = process_manager().write().create_process();
        let address_space = process.address_space();

        let elf = Elf::parse(elf_buffer).expect("Failed to parse application");
        elf.program_headers.iter()
            .filter(|header| header.p_type == elf64::program_header::PT_LOAD)
            .for_each(|header| {
                let page_count = if header.p_memsz as usize % PAGE_SIZE == 0 { header.p_memsz as usize / PAGE_SIZE } else { (header.p_memsz as usize / PAGE_SIZE) + 1 };
                let frames = memory::physical::alloc(page_count);
                let virt_start = Page::from_start_address(VirtAddr::new(header.p_vaddr)).expect("ELF: Program section not page aligned");
                let pages = PageRange { start: virt_start, end: virt_start + page_count as u64 };

                unsafe {
                    let code = elf_buffer.as_ptr().offset(header.p_offset as isize);
                    let target = frames.start.start_address().as_u64() as *mut u8;
                    target.copy_from(code, header.p_filesz as usize);
                    target.offset(header.p_filesz as isize).write_bytes(0, (header.p_memsz - header.p_filesz) as usize);
                }

                process.address_space().map_physical(frames, pages, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
                process.add_vma(VirtualMemoryArea::new(pages, VmaType::Code));
            });

        let kernel_stack = Vec::<u64, StackAllocator>::with_capacity_in((KERNEL_STACK_PAGES * PAGE_SIZE) / 8, StackAllocator::new());
        let user_stack_end = Page::from_start_address(VirtAddr::new((MAIN_USER_STACK_START + MAX_USER_STACK_SIZE) as u64)).unwrap();
        let user_stack_pages = PageRange { start: user_stack_end - 1, end: user_stack_end };
        let user_stack = unsafe { Vec::from_raw_parts_in(user_stack_pages.start.start_address().as_u64() as *mut u64, 0, PAGE_SIZE / 8, StackAllocator::new()) };
        address_space.map(user_stack_pages, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
        process.add_vma(VirtualMemoryArea::new(user_stack_pages, VmaType::Stack));

        let thread = Thread {
            id: scheduler::next_thread_id(),
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process,
            entry: unsafe { mem::transmute(ptr::null::<fn()>()) },
            user_rip: VirtAddr::new(elf.entry)
        };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    pub fn new_user_thread(parent: Arc<Process>, kickoff_addr: VirtAddr, entry: fn()) -> Rc<Thread> {
        let kernel_stack = Vec::<u64, StackAllocator>::with_capacity_in((KERNEL_STACK_PAGES * PAGE_SIZE) / 8, StackAllocator::new());

        let stack_vmas = parent.find_vmas(VmaType::Stack);
        let highest_stack_vma = stack_vmas.last().expect("Trying to create a user thread, before the main thread has been created!");
        let user_stack_end = Page::<Size4KiB>::from_start_address(highest_stack_vma.end() + MAX_USER_STACK_SIZE as u64).unwrap();
        let user_stack_pages = PageRange { start: user_stack_end - 1, end: user_stack_end };
        let user_stack = unsafe { Vec::from_raw_parts_in(user_stack_pages.start.start_address().as_u64() as *mut u64, 0, PAGE_SIZE / 8, StackAllocator::new()) };
        parent.address_space().map(user_stack_pages, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
        parent.add_vma(VirtualMemoryArea::new(user_stack_pages, VmaType::Stack));

        let thread = Thread {
            id: scheduler::next_thread_id(),
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process: parent,
            entry,
            user_rip: kickoff_addr
        };

        thread.prepare_kernel_stack();
        return Rc::new(thread);
    }

    pub fn kickoff_kernel_thread() {
        let scheduler = scheduler();
        scheduler.set_init();

        let thread = scheduler.current_thread();
        tss().lock().privilege_stack_table[0] = thread.kernel_stack_addr();

        if thread.is_kernel_thread() {
            (thread.entry)();
            drop(thread); // Manually decrease reference count, because exit() will never return
            scheduler.exit();
        } else {
            let thread_ptr = ptr::from_ref(thread.as_ref());
            drop(thread); // Manually decrease reference count, because switch_to_user_mode() will never return

            let thread_ref = unsafe { thread_ptr.as_ref().unwrap() };
            thread_ref.switch_to_user_mode();
        }
    }

    pub unsafe fn start_first(thread_ptr: *const Thread) {
        let thread = unsafe { thread_ptr.as_ref().unwrap() };
        let old_rsp0 = thread.stacks.lock().old_rsp0;

        unsafe { thread_kernel_start(old_rsp0.as_u64()); }
    }

    pub unsafe fn switch(current_ptr: *const Thread, next_ptr: *const Thread) {
        let current = unsafe { current_ptr.as_ref().unwrap() };
        let next = unsafe { next_ptr.as_ref().unwrap() };
        let current_rsp0 = ptr::from_ref(&current.stacks.lock().old_rsp0) as *mut u64;
        let next_rsp0 = next.stacks.lock().old_rsp0.as_u64();
        let next_rsp0_end = next.kernel_stack_addr().as_u64();
        let next_address_space = next.process.address_space().page_table_address().as_u64();

        unsafe { thread_switch(current_rsp0, next_rsp0, next_rsp0_end, next_address_space); }
    }

    pub fn is_kernel_thread(&self) -> bool {
        return self.stacks.lock().user_stack.capacity() == 0;
    }

    pub fn stacks_locked(&self) -> bool {
        self.stacks.is_locked()
    }

    pub fn grow_user_stack(&self) {
        let mut stacks = self.stacks.lock();

        // Grow stack area -> Allocate one page right below the stack
        self.process.find_vmas(VmaType::Stack).iter().find(|vma| {
            vma.start().as_u64() == stacks.user_stack.as_ptr() as u64
        }).expect("Failed to find VMA for growing stack").grow_downwards(1);

        // Adapt stack Vec to new start address
        let user_stack_capacity = stacks.user_stack.capacity() + (PAGE_SIZE / 8);
        if user_stack_capacity > MAX_USER_STACK_SIZE / 8 {
            panic!("Stack overflow!");
        }
        
        let user_stack_start = stacks.user_stack.as_ptr() as usize - PAGE_SIZE;
        stacks.user_stack = unsafe { Vec::from_raw_parts_in(user_stack_start as *mut u64, 0, user_stack_capacity, StackAllocator::new()) };
    }

    pub fn user_stack_start(&self) -> VirtAddr {
        let stacks = self.stacks.lock();
        VirtAddr::new(stacks.user_stack.as_ptr() as u64)
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

    pub fn kernel_stack_addr(&self) -> VirtAddr {
        let stacks = self.stacks.lock();
        let kernel_stack_addr = VirtAddr::new(stacks.kernel_stack.as_ptr() as u64);
        return kernel_stack_addr + (stacks.kernel_stack.capacity() * 8) as u64;
    }

    fn prepare_kernel_stack(&self) {
        let mut stacks = self.stacks.lock();
        let stack_addr = stacks.kernel_stack.as_ptr() as u64;
        let capacity = stacks.kernel_stack.capacity();

        for _ in 0..stacks.kernel_stack.capacity() {
            stacks.kernel_stack.push(0);
        }

        stacks.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address
        stacks.kernel_stack[capacity - 2] = Thread::kickoff_kernel_thread as u64; // Address of 'kickoff_kernel_thread()';
        stacks.kernel_stack[capacity - 3] = 0x202; // rflags (Interrupts enabled)

        stacks.kernel_stack[capacity - 4] = 0; // r8
        stacks.kernel_stack[capacity - 5] = 0; // r9
        stacks.kernel_stack[capacity - 6] = 0; // r10
        stacks.kernel_stack[capacity - 7] = 0; // r11
        stacks.kernel_stack[capacity - 8] = 0; // r12
        stacks.kernel_stack[capacity - 9] = 0; // r13
        stacks.kernel_stack[capacity - 10] = 0; // r14
        stacks.kernel_stack[capacity - 11] = 0; // r15

        stacks.kernel_stack[capacity - 12] = 0; // rax
        stacks.kernel_stack[capacity - 13] = 0; // rbx
        stacks.kernel_stack[capacity - 14] = 0; // rcx
        stacks.kernel_stack[capacity - 15] = 0; // rdx

        stacks.kernel_stack[capacity - 16] = 0; // rsi
        stacks.kernel_stack[capacity - 17] = 0; // rdi
        stacks.kernel_stack[capacity - 18] = 0; // rbp

        stacks.old_rsp0 = VirtAddr::new(stack_addr + ((capacity - 18) * 8) as u64);
    }

    fn switch_to_user_mode(&self) {
        let old_rsp0: u64;

        { // Separate block to make sure that the lock is released, before calling `thread_user_start()`.
            let mut stacks = self.stacks.lock();
            let kernel_stack_addr = stacks.kernel_stack.as_ptr() as u64;
            let user_stack_addr = stacks.user_stack.as_ptr() as u64;
            let capacity = stacks.kernel_stack.capacity();

            for _ in 0..stacks.user_stack.capacity() {
                stacks.user_stack.push(0);
            }

            stacks.kernel_stack[capacity - 6] = self.user_rip.as_u64(); // Address of entry point for user thread

            stacks.kernel_stack[capacity - 5] = SegmentSelector::new(4, Ring3).0 as u64; // cs = user code segment
            stacks.kernel_stack[capacity - 4] = 0x202; // rflags (Interrupts enabled)
            stacks.kernel_stack[capacity - 3] = user_stack_addr + (stacks.user_stack.capacity() - 1) as u64 * 8; // rsp for user stack
            stacks.kernel_stack[capacity - 2] = SegmentSelector::new(3, Ring3).0 as u64; // ss = user data segment

            stacks.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address

            stacks.old_rsp0 = VirtAddr::new(kernel_stack_addr + ((capacity - 6) * 8) as u64);
            old_rsp0 = stacks.old_rsp0.as_u64();
        }

        unsafe { thread_user_start(old_rsp0, self.entry); }
    }
}

#[naked]
#[allow(unsafe_op_in_unsafe_fn)]
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
#[allow(unsafe_op_in_unsafe_fn)]
#[allow(improper_ctypes_definitions)] // 'entry' takes no arguments and has no return value, so we just assume that the "C" and "Rust" ABIs act the same way in this case
unsafe extern "C" fn thread_user_start(old_rsp0: u64, entry: fn()) {
    asm!(
    "mov rsp, rdi", // Load 'old_rsp' (first parameter)
    "mov rdi, rsi", // Second parameter becomes first parameter for 'kickoff_user_thread()'
    "iretq", // Switch to user-mode
    options(noreturn)
    )
}

#[naked]
#[allow(unsafe_op_in_unsafe_fn)]
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

    // Set rsp0 of kernel stack in tss (third parameter 'next_rsp0_end')
    "swapgs", // Setup core local storage access via gs base
    "mov rax,gs:[{CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX}]", // Load pointer to rsp0 entry of tss into rax
    "mov [rax],rdx", // Set rsp0 entry in tss to 'next_rsp0_end' (third parameter)
    "swapgs", // Restore gs base

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
    CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX = const CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX,
    options(noreturn)
    )
}