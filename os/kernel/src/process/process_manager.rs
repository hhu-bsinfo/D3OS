/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process manager                                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Functions related to process management.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 20.07.2025                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::info;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::Page;
use x86_64::VirtAddr;

use crate::memory::{vmm, MemorySpace};
use crate::memory::vma::VmaType;
use crate::process::process::Process;
use crate::scheduler;

pub struct ProcessManager {
    active_processes: Vec<Arc<Process>>,
    exited_processes: Vec<Arc<Process>>, // processed by cleanup thread later
}

impl ProcessManager {
    pub const fn new() -> Self {
        Self {
            active_processes: Vec::new(),
            exited_processes: Vec::new(),
        }
    }

    /// Create a new process
    pub fn create_process(&mut self) -> Arc<Process> {
        let kernel_process = self.kernel_process().expect("No kernel process found!");
        let paging = vmm::clone_address_space(&(kernel_process.virtual_address_space));
        let process = Arc::new(Process::new(paging));
        self.active_processes.push(Arc::clone(&process));
        process
    }

    /// Create the kernel process
    pub fn create_kernel_process(&mut self, kernel_image_region: PhysFrameRange, heap_region: PhysFrameRange) -> Arc<Process> {
        let kernel_process = self.kernel_process();
        if kernel_process.is_some() {
            panic!("Kernel process already exists!");
        }

        let paging = vmm::create_kernel_address_space();
        let kernel_process = Arc::new(Process::new(paging));
        self.active_processes.push(Arc::clone(&kernel_process));

        // TODO: adjust this when removing 1:1 mapping
        kernel_process
            .virtual_address_space
            .alloc_vma(
                Some(Page::from_start_address(VirtAddr::new(heap_region.start.start_address().as_u64())).unwrap()),
                heap_region.len(),
                MemorySpace::Kernel,
                VmaType::Heap,
                "heap",
            )
            .expect("failed to create VMA for kernel heap");

        // TODO: stack is part of BSS, which is part of code
        kernel_process
            .virtual_address_space
            .alloc_vma(
                Some(Page::from_start_address(VirtAddr::new(kernel_image_region.start.start_address().as_u64())).unwrap()),
                kernel_image_region.len(),
                MemorySpace::Kernel,
                VmaType::Code,
                "code",
            )
            .expect("failed to create VMA for kernel code");
        kernel_process.dump();

        info!("Kernel process [{}]: created", kernel_process.id());

        kernel_process
    }

    /// Return the ids of all active processes
    pub fn active_process_ids(&self) -> Vec<usize> {
        self.active_processes.iter().map(|process| process.id()).collect()
    }

    /// Get reference to kernel process
    pub fn kernel_process(&self) -> Option<Arc<Process>> {
        self.active_processes.first().map(Arc::clone)
    }

    /// Get reference to current process
    pub fn current_process(&self) -> Arc<Process> {
        if self.active_processes.len() > 1 {
            scheduler().current_thread().process()
        } else {
            self.kernel_process().unwrap()
        }
    }

    /// Exit a process by its id
    pub fn exit(&mut self, process_id: usize) {
        let index = self
            .active_processes
            .iter()
            .position(|process| process.id == process_id)
            .expect("Process: Trying to exit a non-existent process!");

        let process = Arc::clone(&self.active_processes[index]);
        process.kill_all_threads_but_current();

        self.active_processes.swap_remove(index);
        self.exited_processes.push(process);
    }

    /// Kill a process by its id
    pub fn kill(&mut self, process_id: usize) {
        let index = self
            .active_processes
            .iter()
            .position(|process| process.id == process_id)
            .expect("Process: Trying to kill a non-existent process!");

        let process = Arc::clone(&self.active_processes[index]);
        for thread_id in process.thread_ids() {
            scheduler().kill(thread_id);
        }

        self.active_processes.swap_remove(index);
        self.exited_processes.push(process);
    }

    /// 
    pub fn drop_exited_process(&mut self) {
        self.exited_processes.clear();
    }

    /// Dump all active processes
    pub fn dump(&self) {
        info!("=== Active Processes Dump ===");
        if self.active_processes.is_empty() {
            info!("   No active processes.");
            return;
        }

        for (i, process) in self.active_processes.iter().enumerate() {
            info!("Process #{}: PID={}", i, process.id());
            process.virtual_address_space.dump(process.id());
        }
        info!("=============================");
    }
}
