/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process manager                                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Functions related to process management.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 02.03.2025                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::sync::Arc;
use alloc::vec::Vec;
use log::info;

use crate::memory::vmm;
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
        let paging = match self.kernel_process() {
            Some(kernel_process) => {
                // Create user address space
                vmm::clone_address_space(&(kernel_process.virtual_address_space))
            }
            None => vmm::create_kernel_address_space(),
        };

        let process = Arc::new(Process::new(paging));
        self.active_processes.push(Arc::clone(&process));

        info!("Process [{}]: created", process.id());

        process
    }

    pub fn active_process_ids(&self) -> Vec<usize> {
        self.active_processes
            .iter()
            .map(|process| process.id())
            .collect()
    }

    pub fn kernel_process(&self) -> Option<Arc<Process>> {
        match self.active_processes.get(0) {
            Some(kernel_process) => Some(Arc::clone(kernel_process)),
            None => None,
        }
    }

    pub fn current_process(&self) -> Arc<Process> {
        if self.active_processes.len() > 1 {
            scheduler().current_thread().process()
        } else {
            self.kernel_process().unwrap()
        }
    }

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

    pub fn drop_exited_process(&mut self) {
        self.exited_processes.clear();
    }

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
