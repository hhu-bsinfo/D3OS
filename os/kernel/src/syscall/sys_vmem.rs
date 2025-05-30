/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_vmem                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to virtual memory management.          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 24.5.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use x86_64::VirtAddr;
use x86_64::structures::paging::Page;
use log::info;

use crate::memory::vma::VmaType;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process_manager;
use syscall::return_vals::Errno;

/// Map memory to a process.
///
/// This just sets up the VMA, no page tables are created yet.
/// This happens later on on page faults.
pub fn sys_map_memory(start: usize, size: usize) -> isize {
    let process = process_manager().read().current_process();

    let start_addr = VirtAddr::new(start.try_into().unwrap());
    let start_page = Page::containing_address(start_addr);
    let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

    let vma = process.virtual_address_space.alloc_vma(
        Some(start_page),
        num_pages as u64,
        MemorySpace::User,
        VmaType::Heap,
        "heap",
    );
    if vma.is_none() {
        return Errno::EUNKN as isize;
    }
    return 0;
}
