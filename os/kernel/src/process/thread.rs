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
   ║ Author: Fabian Ruhland & Michael Schoettner, 28.6.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use crate::consts::MAIN_USER_STACK_START;
use crate::consts::MAX_USER_STACK_SIZE;
use crate::consts::USER_SPACE_ENV_START;
use crate::memory::stack;
use crate::memory::stack::StackAllocator;
use crate::memory::vma::VmaType;
use crate::memory::PAGE_SIZE;
use crate::process::process::Process;
use crate::process::scheduler;
use crate::syscall::syscall_dispatcher::CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX;
use crate::{process_manager, scheduler, tss};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::naked_asm;
use core::ptr;
use goblin::elf::Elf;
use goblin::elf64;
use log::info;
use spin::Mutex;
use x86_64::instructions::segmentation::{Segment, GS};
use x86_64::PrivilegeLevel::{Ring0, Ring3};
use x86_64::registers::model_specific::FsBase;
use x86_64::registers::segmentation::FS;
use x86_64::VirtAddr;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::paging::Page;

/// kernel & user stack of a thread
struct Stacks {
    kernel_stack: Vec<u64, StackAllocator>,
    user_stack: Vec<u64, StackAllocator>,
    old_rsp0: VirtAddr, // used for thread switching; rsp3 is stored in TSS
}

/// A thread is the unit of execution.
///
/// All threads have a kernel part; you can check whether this thread *just* has
/// a kernel part by calling [`Thread::is_kernel_thread`].
///
/// Threads can be created in the following ways:
/// * [`Thread::new_kernel_thread`]: for kernel threads
/// * [`Thread::load_application`]: for the main thread of an application
/// * [`Thread::new_user_thread`]: for additional threads of an application
///
/// This will allocate all required ressources, but will not actually start the
/// thread. You need to call [`scheduler::Scheduler::ready`] to enqueue it.
///
/// When the scheduler first switches to the new thread, it will start with
/// [`Thread::kickoff_kernel_thread`]. This sets up the TSS and then:
/// * for a kernel thread: call the `entry` function,
///   and [`scheduler::Scheduler::exit`] afterwards.
/// * for a user thread: call `user_kickoff(entry)`,
///   with `user_kickoff` being `library::concurrent::thread::kickoff_user_thread`.
///   This is needed so that the actual `entry` function of the application
///   can safely return.
pub struct Thread {
    id: usize,
    stacks: Mutex<Stacks>,
    process: Arc<Process>, // reference to my process
    /// for user threads: the address to jump to
    user_kickoff: VirtAddr,
    /// for user threads: pointer to the thread environment block (e.g. for local storage), which is loaded into fs_base
    user_environment: VirtAddr,
    /// the actual entry point (eg. for user threads the single parameter to kickoff)
    entry: extern "sysv64" fn(),
}

impl Stacks {
    const fn new(kernel_stack: Vec<u64, StackAllocator>, user_stack: Vec<u64, StackAllocator>) -> Self {
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
    pub fn new_kernel_thread(entry: extern "sysv64" fn(), tag_str: &str) -> Arc<Thread> {
        let process = process_manager().read().current_process();
        let pid = process.id();
        let tid = scheduler::next_thread_id();

        // Allocate the kernel stack for the kernel thread
        let kernel_stack = stack::alloc_kernel_stack(&process, pid, tid, tag_str);

        // Create empty user stack, so need to add it to the virtual address space
        let user_stack: Vec<u64, StackAllocator> = stack::alloc_user_stack(pid, tid, MAIN_USER_STACK_START, 0);

        // Create the thread struct
        let thread = Thread {
            id: tid,
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process: process_manager()
                .read()
                .kernel_process()
                .expect("Trying to create a kernel thread before process initialization!"),
            user_kickoff: VirtAddr::zero(),
            user_environment: VirtAddr::zero(),
            entry,
        };

        thread.prepare_kernel_stack();
        Arc::new(thread)
    }

    /// Load application code from `elf_buffer`, create a process with a main thread. \
    /// `name` is the name of the application, `args` are the arguments passed to the application. \
    /// Returns the main thread of the application which is not yet registered in the scheduler.
    pub fn load_application(elf_buffer: &[u8], name: &str, args: &Vec<&str>) -> Arc<Thread> {
        let current_process = process_manager().read().current_process();
        let new_process = process_manager().write().create_process();
        let pid = new_process.id();
        let tid = scheduler::next_thread_id();

        info!("load_application: pid = {pid}, tid = {tid}, name = {name}",);

        // parse elf file headers and map and copy code if successful
        let entry = Thread::parse_and_map_elf_bin(&current_process, &new_process, elf_buffer, name);

        // create environment for the application and copy arguments
        Thread::copy_args(&new_process, name, args);

        // create thread
        // this first thread is special in that there is not really a kickoff;
        // we just jump to the ELF's entry point
        // TODO: this leaks a kernel address to user space
        extern "sysv64" fn entry_fn() {
            unreachable!()
        }
        Self::new_user_thread(new_process, VirtAddr::new(entry), entry_fn)
    }

    /// Create user thread. Not started yet, nor registered in the scheduler. \
    /// `parent` is the process the thread belongs to. \
    /// `kickoff_addr` address of the first function to be called,
    /// with the `entry` function is the parameter. \
    /// This indirection ensures that the thread calls exit when it is done, see `library::concurrent::thread`.
    pub fn new_user_thread(
        parent: Arc<Process>,
        kickoff_addr: VirtAddr,
        entry: extern "sysv64" fn(),
    ) -> Arc<Thread> {
        let pid = parent.id();
        let tid = scheduler::next_thread_id(); // get id for new thread

        // Allocate kernel stack for the main thread
        let kernel_stack = stack::alloc_kernel_stack(&parent, pid, tid, "userthread");

        // Create user stack for the application
        let stack_vma = parent.virtual_address_space.user_alloc_map_partial(None, (MAX_USER_STACK_SIZE / PAGE_SIZE) as u64,  VmaType::UserStack, "usrstack", 1, true).expect("could not create user stack");

        // Make a Vec for the user stack
        let user_stack: Vec<u64, StackAllocator> = stack::alloc_user_stack(pid, tid, stack_vma.start().as_u64() as usize, MAX_USER_STACK_SIZE);

        // Create environment for the thread
        let user_environment = parent.virtual_address_space
            .user_alloc_map_full(None, 1, VmaType::Environment, "threadenv")
            .expect("could not create thread environment");

        unsafe {
            // First entry in user thread environment is a self referencing pointer
            let environment_ptr = parent.virtual_address_space
                .get_phys(user_environment.start().as_u64())
                .unwrap()
                .as_u64() as *mut u64;

            environment_ptr.write(user_environment.start().as_u64());
        }

        // create user thread and prepare the stack for starting it later
        let thread = Thread {
            id: tid,
            stacks: Mutex::new(Stacks::new(kernel_stack, user_stack)),
            process: parent,
            user_kickoff: kickoff_addr,
            user_environment: user_environment.start(),
            entry,
        };

        thread.prepare_kernel_stack();
        Arc::new(thread)
    }

    /// Called first for both a new kernel and a new user thread
    fn kickoff_kernel_thread() -> ! {
        let scheduler = scheduler();
        scheduler.set_init(); // scheduler initialized

        let thread = scheduler.current_thread();
        tss().lock().privilege_stack_table[0] = thread.kernel_stack_addr(); // get stack pointer for kernel stack

        if thread.is_kernel_thread() {
            assert!(thread.user_kickoff.is_null());
            (thread.entry)(); // Directly call the entry function of kernel thread
            drop(thread); // Manually decrease reference count, because exit() will never return
            scheduler.exit();
        } else {
            assert!(!thread.user_kickoff.is_null());
            let thread_ptr = ptr::from_ref(thread.as_ref());
            drop(thread); // Manually decrease reference count, because switch_to_user_mode() will never return

            let thread_ref = unsafe { thread_ptr.as_ref().unwrap() };
            thread_ref.switch_to_user_mode(); // call kickoff function of user thread
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
        let next_address_space = next.process.virtual_address_space.page_table_address().as_u64();

        unsafe {
            thread_switch(current_rsp0, next_rsp0, next_rsp0_end, next_address_space);
        }
    }

    /// Check if stacks are locked
    pub fn stacks_locked(&self) -> bool {
        self.stacks.is_locked()
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

        // init stack with 0s
        for _ in 0..stacks.kernel_stack.capacity() {
            stacks.kernel_stack.push(0);
        }
        let top_of_stack = Self::get_top_of_stack(&stacks.kernel_stack);
        let capacity = stacks.kernel_stack.capacity();
        let segment_selector = SegmentSelector::new(2, Ring0).0 as u64; // kernel data segment

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

        stacks.kernel_stack[capacity - 19] = 0; // fs base
        stacks.kernel_stack[capacity - 20] = (segment_selector << 48 | segment_selector << 32) as u64; // fs and gs

        stacks.old_rsp0 = VirtAddr::new((top_of_stack as usize - (8 * 19) - 4) as u64);
    }

    /// Switch a thread to user mode by preparing a fake stackframe
    fn switch_to_user_mode(&self) -> ! {
        let old_rsp0: u64;

        {
            // Separate block to make sure that the lock is released, before calling `thread_user_start()`.
            let mut stacks = self.stacks.lock();
            let kernel_stack_addr = stacks.kernel_stack.as_ptr() as u64;
            let user_stack_addr = stacks.user_stack.as_ptr() as u64;
            let capacity = stacks.kernel_stack.capacity();

            stacks.kernel_stack[capacity - 6] = self.user_kickoff.as_u64(); // Address of entry point for user thread

            stacks.kernel_stack[capacity - 5] = SegmentSelector::new(4, Ring3).0 as u64; // cs = user code segment
            stacks.kernel_stack[capacity - 4] = 0x202; // rflags (Interrupts enabled)
            stacks.kernel_stack[capacity - 3] = user_stack_addr + (stacks.user_stack.capacity() - 1) as u64 * 8; // rsp for user stack
            stacks.kernel_stack[capacity - 2] = SegmentSelector::new(3, Ring3).0 as u64; // ss = user data segment

            stacks.kernel_stack[capacity - 1] = 0x00DEAD00u64; // Dummy return address

            stacks.old_rsp0 = VirtAddr::new(kernel_stack_addr + ((capacity - 6) * 8) as u64);
            old_rsp0 = stacks.old_rsp0.as_u64();
        }

        unsafe {
            FS::set_reg(SegmentSelector::new(4, Ring3));
            GS::set_reg(SegmentSelector::new(4, Ring3));
            FsBase::write(self.user_environment);

            thread_user_start(old_rsp0, self.entry);
        }
    }

    /// Helper function to parse ELF binary and map it into the new process's address space
    /// Used only by `load_application()`
    fn parse_and_map_elf_bin(current_process: &Arc<Process>, new_process: &Arc<Process>, elf_buffer: &[u8], name: &str) -> u64 {
        let elf = Elf::parse(elf_buffer).expect("Failed to parse application");
        elf.program_headers
            .iter()
            .filter(|header| header.p_type == elf64::program_header::PT_LOAD)
            .for_each(|header| {
                // Calc total number of pages for .text and .bss = 'p_memsz'
                let total_page_count = if header.p_memsz as usize % PAGE_SIZE == 0 {
                    header.p_memsz as usize / PAGE_SIZE
                } else {
                    (header.p_memsz as usize / PAGE_SIZE) + 1
                };

                // Calc number of pages needed for the .text section = 'p_filesz'
                let code_page_count = if header.p_filesz as usize % PAGE_SIZE == 0 {
                    header.p_filesz as usize / PAGE_SIZE
                } else {
                    (header.p_filesz as usize / PAGE_SIZE) + 1
                };

                // create mapping for 'total_page_count'
                let virt_start = Page::from_start_address(VirtAddr::new(header.p_vaddr)).expect("ELF: Program section not page aligned");
                let vma = new_process
                    .virtual_address_space
                    .user_alloc_map_full(Some(virt_start), total_page_count as u64, VmaType::Code, name)
                    .expect("user_alloc_map_full failed");

                // copy code from the ELF file to the allocated frames
                // as the target address space is not loaded we need to copy page by page by retrieving physical addresses manually from page tables of the target process
                unsafe {
                    let src_ptr = elf_buffer.as_ptr().offset(header.p_offset as isize);
                    current_process.virtual_address_space.copy_to_addr_space(
                        src_ptr,
                        &new_process.virtual_address_space,
                        vma.range.start,
                        header.p_filesz,
                        true,
                    );
                }

                // Zero remaining pages for .bss
                if total_page_count > code_page_count {
                    let bss_page_count = total_page_count - code_page_count;
                    let dest_page_start = vma.range.start.start_address().as_u64();
                    let mut dest_offset: u64 = code_page_count as u64 * PAGE_SIZE as u64;

                    // copy remaining pages
                    for _i in 0..bss_page_count {
                        // get destination physical address
                        let dest_phys_addr = new_process
                            .virtual_address_space
                            .get_phys(dest_page_start + dest_offset)
                            .expect("get_phys failed");
                        let dest = dest_phys_addr.as_u64() as *mut u8;

                        // zero rest of the page
                        unsafe {
                            dest.write_bytes(0, PAGE_SIZE);
                        }
                        dest_offset += PAGE_SIZE as u64;
                    }
                }
            });

        elf.entry
    }

    /// Helper function to provide arguments to a new application
    /// Used only by `load_application()`
    fn copy_args(new_process: &Arc<Process>, name: &str, args: &Vec<&str>) {
        let args_size = args.iter().map(|arg| arg.len()).sum::<usize>();
        let env_size = args_size;

        let env_virt_start = Page::from_start_address(VirtAddr::new(USER_SPACE_ENV_START as u64)).unwrap();
        let env_page_count = if env_size > 0 && env_size % PAGE_SIZE == 0 {
            env_size / PAGE_SIZE
        } else {
            (env_size / PAGE_SIZE) + 1
        };

        // create mapping for 'total_page_count'
        let _vma = new_process
            .virtual_address_space
            .user_alloc_map_full(Some(env_virt_start), env_page_count as u64, VmaType::Environment, "env")
            .expect("user_alloc_map_full failed");

        if env_page_count > 1 {
            panic!("Environment size exceeds one page, which is not supported yet");
        }

        let env_frame = new_process.virtual_address_space.get_phys(env_virt_start.start_address().as_u64())
            .expect("get_phys failed for environment");

        // create argc and argv in the user space environment
        let env_addr = VirtAddr::new(env_frame.as_u64()); // Start address of user space environment
        let argc = env_addr.as_mut_ptr::<usize>(); // First entry in environment is argc (number of arguments)
        let argv = (env_addr + size_of::<usize>() as u64).as_mut_ptr::<*const u8>(); // Second entry in environment is argv (array of pointers to arguments)

        // copy arguments directly behind argv array and store pointers to them in argv
        unsafe {
            argc.write(args.len() + 1);

            let args_begin = argv.add(args.len() + 1) as *mut u8; // Physical start address of arguments (we use this address to copy them)
            let args_begin_virt = env_virt_start.start_address() + size_of::<usize>() as u64 + ((args.len() + 1) * size_of::<usize>()) as u64; // Virtual start address of arguments (they will be visible here in user space)

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

                argv.add(i + 1).write((args_begin_virt + offset as u64).as_ptr());
                offset += arg.len() + 1;
            }
        }
    }

    /// Get a pointer to the top of the given stack.
    fn get_top_of_stack(stack: &Vec<u64, StackAllocator>) -> *const u64 {
        unsafe {
            ptr::from_ref(&stack[stack.len() - 1]).offset(1)
        }
    }
}

/// Low-level function for starting a thread in kernel mode
#[unsafe(naked)]
unsafe extern "C" fn thread_kernel_start(old_rsp0: u64) {
    naked_asm!(
        "mov rsp, rdi", // First parameter -> load 'old_rsp0'
        "pop gs",
        "pop fs",
        "pop rax", "wrfsbase rax",
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
unsafe extern "C" fn thread_user_start(old_rsp0: u64, entry: extern "sysv64" fn()) -> ! {
    naked_asm!(
        "mov rsp, rdi", // Load 'old_rsp' (first parameter)
        "mov rdi, rsi", // Second parameter becomes first parameter for 'kickoff_user_thread()'
        "iretq"         // Switch to user-mode
    )
}

/// Low-level thread switching function
#[unsafe(naked)]
unsafe extern "C" fn thread_switch(current_rsp0: *mut u64, next_rsp0: u64, next_rsp0_end: u64, next_cr3: u64) {
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
    "rdfsbase rax", "push rax",
    "push fs",
    "push gs",

    // Save stack pointer in 'current_rsp0' (first parameter)
    "mov [rdi], rsp",

    // Set rsp0 of kernel stack in tss (third parameter 'next_rsp0_end')
    "mov ax, 0x10", // Load segment selector for kernel data segment
    "mov gs, ax",
    "swapgs", // Setup core local storage access via gs base
    "mov rax, gs:[{CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX}]", // Load pointer to rsp0 entry of tss into rax
    "mov [rax], rdx", // Set rsp0 entry in tss to 'next_rsp0_end' (third parameter)
    "swapgs", // Restore gs base

    // Switch address space (fourth parameter 'next_cr3')
    "mov cr3, rcx",

    // Load registers of next thread by using 'next_rsp0' (second parameter)
    "mov rsp, rsi",
    "pop gs",
    "pop fs",
    "pop rax", "wrfsbase rax",
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
    CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX = const CORE_LOCAL_STORAGE_TSS_RSP0_PTR_INDEX,
    )
}
