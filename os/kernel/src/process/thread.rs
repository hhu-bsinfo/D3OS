/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: thread                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Implementation of threads. Supports kernel-only threads as well as      ║
   ║ several user thread per process.                                        ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║  - new_kernel_thread  create a new kernel-only thread                   ║
   ║  - load_application   load application, create process, and main thread ║
   ║  - new_user_thread    create and additional user thread in a process    ║
   ║  - start_first        start a thread, called once by scheduler          ║
   ║  - switch             switch threads, called by scheduler               ║
   ║  - stacks_locked      check if stacks are locked, called by scheduler   ║
   ║  - grow_user_stack    grow stack as needed, called from page fault      ║
   ║  - user_stack_start   return last usable address of user stack          ║
   ║  - is_kernel_thread   check if self is a kernel only thread or not      ║
   ║  - process            return reference to my process                    ║
   ║  - id                 return my thread id                               ║
   ║  - join               calling thread will wait until 'self' terminates  ║
   ║                                                                         ║
   ║ Thread stack:                                                           ║
   ║  Kernel threads have a stack of 'KERNEL_STACK_PAGES'. User threads have ║
   ║  an additional stack with a logical size of 'MAX_USER_STACK_SIZE' and   ║
   ║  an initial phyiscal size of one page. Additional pages are allocated   ║
   ║  for user stacks as need until 'MAX_USER_STACK_SIZE' is reached.        ║
   ║  A thread is killed if this limit is exceeded. The stack of a user      ║
   ║  thread stack within one processes is logically allocated at            ║
   ║  'MAIN_USER_STACK_START'. The next stack for the next user stack is     ║
   ║  allocated at 'MAIN_USER_STACK_START' + 'MAX_USER_STACK_SIZE' and so on.║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, HHU                                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use crate::consts::MAIN_USER_STACK_START;
use crate::consts::MAX_USER_STACK_SIZE;
use crate::consts::{KERNEL_STACK_PAGES, USER_SPACE_ENV_START};
use crate::memory::stack::StackAllocator;
use crate::memory::vmm::VirtualMemoryArea;
use crate::memory::vmm::VmaType;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process::process::Process;
use crate::process::scheduler;
use crate::syscall::syscall_dispatcher::CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX;
use crate::{memory, process_manager, scheduler, tss};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::naked_asm;
use core::ptr;
use goblin::elf::Elf;
use goblin::elf64;
use log::info;
use spin::Mutex;
use x86_64::PrivilegeLevel::Ring3;
use x86_64::VirtAddr;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB};


/// kernel & user stack of a thread
struct Stacks {
    kernel_stack: Vec<u64, StackAllocator>,
    user_stack: Vec<u64, StackAllocator>,
    old_rsp0: VirtAddr, // used for thread switching; rsp3 is stored in TSS
}

/// Thread meta data
pub struct Thread {
    id: usize,
    stacks: Mutex<Stacks>,
    process: Arc<Process>, // reference to my process
    entry: fn(), // user thread: =0;                 kernel thread: address of entry function
    user_rip: VirtAddr, // user thread: elf-entry function; kernel thread: =0
}

impl Stacks {
    const fn new(
        kernel_stack: Vec<u64, StackAllocator>,
        user_stack: Vec<u64, StackAllocator>,
    ) -> Self {
        Self {
            kernel_stack,
            user_stack,
            old_rsp0: VirtAddr::zero(),
        }
    }
}

impl Thread {
    /// Create a kernel thread. Not started yet, nor registered in the scheduler. \
    /// `entry` is the thread entry function.
    pub fn new_kernel_thread(entry: fn(), tag_str: &str) -> Arc<Thread> {
        // alocate frames for kernel stack
        let kernel_stack = Vec::<u64, StackAllocator>::with_capacity_in(
            (KERNEL_STACK_PAGES * PAGE_SIZE) / 8,
            StackAllocator::default(),
        );

        // add kernel stack to the virtual address space
        process_manager()
            .read()
            .current_process()
            .virtual_address_space
            .map_kernel_stack(
                PageRange {
                    start: Page::from_start_address(VirtAddr::new(kernel_stack.as_ptr() as u64))
                        .unwrap(),
                    end: Page::from_start_address(VirtAddr::new(
                        kernel_stack.as_ptr() as u64 + kernel_stack.capacity() as u64 * 8,
                    ))
                    .unwrap(),
                },
                tag_str,
            );

        // empty user stack, so need to add it to the virtual address space
        let user_stack = Vec::with_capacity_in(0, StackAllocator::default()); // Dummy stack

        let thread = Thread {
            id: scheduler::next_thread_id(),
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process: process_manager()
                .read()
                .kernel_process()
                .expect("Trying to create a kernel thread before process initialization!"),
            entry,
            user_rip: VirtAddr::zero(),
        };

        thread.prepare_kernel_stack();
        Arc::new(thread)
    }

    /// Load application code from `elf_buffer`, create a process with a main thread. \
    /// `name` is the name of the application, `args` are the arguments passed to the application. \
    /// Returns the main thread of the application which is not yet registered in the scheduler.
    pub fn load_application(elf_buffer: &[u8], name: &str, args: &Vec<&str>) -> Arc<Thread> {
        let process = process_manager().write().create_process();

        // Parse elf file headers and map code vma if successful
        let elf = Elf::parse(elf_buffer).expect("Failed to parse application");
        elf.program_headers
            .iter()
            .filter(|header| header.p_type == elf64::program_header::PT_LOAD)
            .for_each(|header| {
                let page_count = if header.p_memsz as usize % PAGE_SIZE == 0 {
                    header.p_memsz as usize / PAGE_SIZE
                } else {
                    (header.p_memsz as usize / PAGE_SIZE) + 1
                };

                let virt_start = Page::from_start_address(VirtAddr::new(header.p_vaddr))
                    .expect("ELF: Program section not page aligned");
                let pages = PageRange {
                    start: virt_start,
                    end: virt_start + page_count as u64,
                };

                // Map pages for the code and allocate frames for all pages
                let frames = process.virtual_address_space.map_and_allocate(
                    pages,
                    pages,
                    MemorySpace::User,
                    PageTableFlags::PRESENT
                        | PageTableFlags::WRITABLE
                        | PageTableFlags::USER_ACCESSIBLE,
                    VmaType::Code,
                    name,
                );

                // copy code from the ELF file to the allocated frames
                unsafe {
                    let code = elf_buffer.as_ptr().offset(header.p_offset as isize);
                    let target = frames.start.start_address().as_u64() as *mut u8;
                    target.copy_from(code, header.p_filesz as usize);
                    target
                        .offset(header.p_filesz as isize)
                        .write_bytes(0, (header.p_memsz - header.p_filesz) as usize);
                }
            });

//        info!("*** load_application: mapped code vma");

        
        // Create kernel stack for the application 
        // Does not require mapping, because full physical memory is mapped 1:1 in kernel space
        info!("   create kernel stack");
        let kernel_stack = Vec::<u64, StackAllocator>::with_capacity_in(
            (KERNEL_STACK_PAGES * PAGE_SIZE) / 8,
            StackAllocator::default(),
        );



        // create user stack for the application
        let user_stack_end = Page::from_start_address(VirtAddr::new(
            (MAIN_USER_STACK_START + MAX_USER_STACK_SIZE) as u64,
        ))
        .unwrap();
    
        // last page of the user stack
        let user_stack_pages = PageRange {
            start: user_stack_end - 1,
            end: user_stack_end,
        };

        // Create Vec for user stack (backed by stack allocator)
        let user_stack = unsafe {
            Vec::from_raw_parts_in(
                user_stack_pages.start.start_address().as_u64() as *mut u64,
                0,
                PAGE_SIZE / 8,
                StackAllocator::default(),
            )
        };

        // map user stack of the application
        info!("   map user stack: {:x?}, #pages: {:?}", user_stack_pages, user_stack_pages.len());
        process.virtual_address_space.map(
            user_stack_pages,
            MemorySpace::User,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            VmaType::UserStack,
            "",
        );

//        info!("Created user stack for thread: {:x?}", user_stack_pages);



        // create environment for the application
        let args_size = args.iter().map(|arg| arg.len()).sum::<usize>();
        let env_virt_start =
            Page::from_start_address(VirtAddr::new(USER_SPACE_ENV_START as u64)).unwrap();
        let env_size = args_size;
        let env_page_count = if env_size > 0 && env_size % PAGE_SIZE == 0 {
            env_size / PAGE_SIZE
        } else {
            (env_size / PAGE_SIZE) + 1
        };
        let env_frames = memory::frames::alloc(env_page_count);
        let env_pages = PageRange {
            start: env_virt_start,
            end: env_virt_start + env_page_count as u64,
        };

        // map environment of the application
        process.virtual_address_space.map_physical(
            env_frames,
            env_pages,
            MemorySpace::User,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            VmaType::Environment,
            "",
        );




        // create argc and argv in the user space environment
        let env_addr = VirtAddr::new(env_frames.start.start_address().as_u64()); // Start address of user space environment
        let argc = env_addr.as_mut_ptr::<usize>(); // First entry in environment is argc (number of arguments)
        let argv = (env_addr + size_of::<usize>() as u64).as_mut_ptr::<*const u8>(); // Second entry in environment is argv (array of pointers to arguments)

        // copy arguments directly behind argv array and store pointers to them in argv
        unsafe {
            argc.write(args.len() + 1);

            let args_begin = argv.offset((args.len() + 1) as isize) as *mut u8; // Physical start address of arguments (we use this address to copy them)
            let args_begin_virt = env_virt_start.start_address()
                + size_of::<usize>() as u64
                + ((args.len() + 1) * size_of::<usize>()) as u64; // Virtual start address of arguments (they will be visible here in user space)

            // copy program name as first argument
            args_begin.copy_from(name.as_bytes().as_ptr(), name.len());
            args_begin.add(name.len()).write(0); // null-terminate the string for C compatibility
            argv.write(args_begin_virt.as_ptr());

            let mut offset = name.len() + 1;

            // copy remaining arguments
            for (i, arg) in args.iter().enumerate() {
                let target = args_begin.add(offset);
                target.copy_from(arg.as_bytes().as_ptr(), arg.len());
                target.add(arg.len()).write(0); // null-terminate the string for C compatibility

                argv.add(i + 1)
                    .write((args_begin_virt + offset as u64).as_ptr());
                offset += arg.len() + 1;
            }
        }

        // create thread
        let thread = Thread {
            id: scheduler::next_thread_id(),
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process,
            entry: || {},
            // old           entry: unsafe { mem::transmute(ptr::null::<fn()>()) },
            user_rip: VirtAddr::new(elf.entry),
        };

        thread.prepare_kernel_stack();
        Arc::new(thread)
    }

    /// Create user thread. Not started yet, nor registered in the scheduler. \
    /// `parent` is the process the thread belongs to. \
    /// `kickoff_addr` address of the first function to be called before the thread `entry` function is executed. \
    /// This indirection ensures that the thread calls exit when it is done, see `library::concurrent::thread`.
    pub fn new_user_thread(
        parent: Arc<Process>,
        kickoff_addr: VirtAddr,
        entry: fn(),
    ) -> Arc<Thread> {
        // alloc memory for kernel stack
        let kernel_stack = Vec::<u64, StackAllocator>::with_capacity_in(
            (KERNEL_STACK_PAGES * PAGE_SIZE) / 8,
            StackAllocator::default(),
        );

        // get highest stack vma in my address space
        let stack_vmas = parent.virtual_address_space.find_vmas(VmaType::UserStack);
        let highest_stack_vma = stack_vmas
            .last()
            .expect("Trying to create a user thread, before the main thread has been created!");

        // from there allocate new user stack
        let user_stack_end = Page::<Size4KiB>::from_start_address(
            highest_stack_vma.end() + MAX_USER_STACK_SIZE as u64,
        )
        .unwrap();
        let user_stack_pages = PageRange {
            start: user_stack_end - 1,
            end: user_stack_end,
        };

        let user_stack = unsafe {
            Vec::from_raw_parts_in(
                user_stack_pages.start.start_address().as_u64() as *mut u64,
                0,
                PAGE_SIZE / 8,
                StackAllocator::default(),
            )
        };

        // id for new thread
        let tid = scheduler::next_thread_id();

        // map one page as PRESENT for the allocated user stack
        parent.virtual_address_space.map(
            user_stack_pages,
            MemorySpace::User,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
            VmaType::UserStack,
            "",
        );

        // create user thread and prepare the stack for starting it later
        let thread = Thread {
            id: tid,
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process: parent,
            entry,
            user_rip: kickoff_addr,
        };

        info!("Created user stack for thread: {:x?}", user_stack_pages);

        thread.prepare_kernel_stack();
        Arc::new(thread)
    }

    /// Called first for both a new kernel and a new user thread
    fn kickoff_kernel_thread() {
        let scheduler = scheduler();
        scheduler.set_init(); // scheduler initialized

        let thread = scheduler.current_thread();
        tss().lock().privilege_stack_table[0] = thread.kernel_stack_addr(); // get stack pointer for kernel stack

        if thread.is_kernel_thread() {
            (thread.entry)(); // Directly call the entry function of kernel thread
            drop(thread); // Manually decrease reference count, because exit() will never return
            scheduler.exit();
        } else {
            let thread_ptr = ptr::from_ref(thread.as_ref());
            drop(thread); // Manually decrease reference count, because switch_to_user_mode() will never return

            let thread_ref = unsafe { thread_ptr.as_ref().unwrap() };
            thread_ref.switch_to_user_mode(); // call entry function of user thread
            // exit is in the entry function -> runtime::lib.rs
        }
    }

    /// High-level function for starting a thread in kernel mode
    pub unsafe fn start_first(thread_ptr: *const Thread) {
        let thread = unsafe { thread_ptr.as_ref().unwrap() };
        let old_rsp0 = thread.stacks.lock().old_rsp0;

        unsafe {
            thread_kernel_start(old_rsp0.as_u64());
        }
    }

    /// High-level thread switching function
    pub unsafe fn switch(current_ptr: *const Thread, next_ptr: *const Thread) {
        let current = unsafe { current_ptr.as_ref().unwrap() };
        let next = unsafe { next_ptr.as_ref().unwrap() };
        let current_rsp0 = ptr::from_ref(&current.stacks.lock().old_rsp0) as *mut u64;
        let next_rsp0 = next.stacks.lock().old_rsp0.as_u64();
        let next_rsp0_end = next.kernel_stack_addr().as_u64();
        let next_address_space = next
            .process
            .virtual_address_space
            .page_table_address()
            .as_u64();

        unsafe {
            thread_switch(current_rsp0, next_rsp0, next_rsp0_end, next_address_space);
        }
    }

    /// Check if stacks are locked
    pub fn stacks_locked(&self) -> bool {
        self.stacks.is_locked()
    }

    /// Grow user stack on demand
    pub fn grow_user_stack(&self) {
        let mut stacks = self.stacks.lock();

        // Grow stack area -> Allocate one page right below the stack
        self.process
            .virtual_address_space
            .find_vmas(VmaType::UserStack)
            .iter()
            .find(|vma| vma.start().as_u64() == stacks.user_stack.as_ptr() as u64)
            .expect("Failed to find VMA for growing stack")
            .grow_downwards(1);

        // Adapt stack Vec to new start address
        let user_stack_capacity = stacks.user_stack.capacity() + (PAGE_SIZE / 8);
        if user_stack_capacity > MAX_USER_STACK_SIZE / 8 {
            panic!("Stack overflow!");
        }

        let user_stack_start = stacks.user_stack.as_ptr() as usize - PAGE_SIZE;
        // Fix me 
        stacks.user_stack = unsafe {
            Vec::from_raw_parts_in(
                user_stack_start as *mut u64,
                0,
                user_stack_capacity,
                StackAllocator::default(),
            )
        };
    }

    /// Check if self is a kernel only thread or not
    pub fn is_kernel_thread(&self) -> bool {
        self.stacks.lock().user_stack.capacity() == 0
    }

    /// Return last usable address of user stack. Used to implement dynamically growing stack
    pub fn user_stack_start(&self) -> VirtAddr {
        let stacks = self.stacks.lock();
        VirtAddr::new(stacks.user_stack.as_ptr() as u64)
    }

    /// Return reference to my process
    pub fn process(&self) -> Arc<Process> {
        Arc::clone(&self.process)
    }

    /// Calling thread will wait until 'self' terminates
    #[allow(dead_code)]
    pub fn join(&self) {
        scheduler().join(self.id());
    }

    /// Return my thread id
    pub fn id(&self) -> usize {
        self.id
    }

    /// Helper function, returns highest useable stack address of kernel stack  of 'self'
    fn kernel_stack_addr(&self) -> VirtAddr {
        let stacks = self.stacks.lock();
        let kernel_stack_addr = VirtAddr::new(stacks.kernel_stack.as_ptr() as u64);
        kernel_stack_addr + (stacks.kernel_stack.capacity() * 8) as u64
    }

    /// Prepare a fake stack for starting a thread in kernel mode
    fn prepare_kernel_stack(&self) {
        let mut stacks = self.stacks.lock();
        let stack_addr = stacks.kernel_stack.as_ptr() as u64;
        let capacity = stacks.kernel_stack.capacity();

        // init stack with 0s
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

    /// Switch a thread to user mode by preparing a fake stackframe
    fn switch_to_user_mode(&self) {
        let old_rsp0: u64;

        {
            // Separate block to make sure that the lock is released, before calling `thread_user_start()`.
            let mut stacks = self.stacks.lock();
            let kernel_stack_addr = stacks.kernel_stack.as_ptr() as u64;
            let user_stack_addr = stacks.user_stack.as_ptr() as u64;
            let capacity = stacks.kernel_stack.capacity();

            // init stack with 0s
            for _ in 0..stacks.user_stack.capacity() {
                stacks.user_stack.push(0);
            }

            stacks.kernel_stack[capacity - 6] = self.user_rip.as_u64(); // Address of entry point for user thread

            stacks.kernel_stack[capacity - 5] = SegmentSelector::new(4, Ring3).0 as u64; // cs = user code segment
            stacks.kernel_stack[capacity - 4] = 0x202; // rflags (Interrupts enabled)
            stacks.kernel_stack[capacity - 3] =
                user_stack_addr + (stacks.user_stack.capacity() - 1) as u64 * 8; // rsp for user stack
            stacks.kernel_stack[capacity - 2] = SegmentSelector::new(3, Ring3).0 as u64; // ss = user data segment

            stacks.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address

            stacks.old_rsp0 = VirtAddr::new(kernel_stack_addr + ((capacity - 6) * 8) as u64);
            old_rsp0 = stacks.old_rsp0.as_u64();
        }

        unsafe {
            thread_user_start(old_rsp0, self.entry);
        }
    }
}

/// Low-level function for starting a thread in kernel mode
#[unsafe(naked)]
unsafe extern "C" fn thread_kernel_start(old_rsp0: u64) {
    naked_asm!(
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
        "call unlock_scheduler", // force unlock, thread_switch locks Scheduler but returns later
        "ret"
    );
}

/// Low-level function for starting a thread in user mode
#[unsafe(naked)]
#[allow(improper_ctypes_definitions)] // 'entry' takes no arguments and has no return value, so we just assume that the "C" and "Rust" ABIs act the same way in this case
unsafe extern "C" fn thread_user_start(old_rsp0: u64, entry: fn()) {
    naked_asm!(
        "mov rsp, rdi", // Load 'old_rsp' (first parameter)
        "mov rdi, rsi", // Second parameter becomes first parameter for 'kickoff_user_thread()'
        "iretq"         // Switch to user-mode
    )
}

/// Low-level thread switching function
#[unsafe(naked)]
unsafe extern "C" fn thread_switch(
    current_rsp0: *mut u64,
    next_rsp0: u64,
    next_rsp0_end: u64,
    next_cr3: u64,
) {
    naked_asm!(
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

    "call unlock_scheduler", // force unlock, thread_switch locks Scheduler but returns later
    "ret", // Return to next thread
    CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX = const CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX
    )
}
