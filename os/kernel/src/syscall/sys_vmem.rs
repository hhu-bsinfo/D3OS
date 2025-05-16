/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_vmem                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to virtual memory management.          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use x86_64::VirtAddr;

use crate::memory::vmm::{VirtualMemoryArea, VmaType};
use crate::process_manager;


/// Map memory to a process.
/// 
/// This just sets up the VMA, no page tables are created yet.
/// This happens later on on page faults.
pub fn sys_map_memory(start: usize, size: usize) -> isize {
    let start_addr = VirtAddr::new(start.try_into().unwrap());
    let process = process_manager().read().current_process();

    let area = VirtualMemoryArea::from_address(start_addr, size, VmaType::Heap);
    // insert it into the process, this checks if it's free
    process.virtual_address_space.add_vma(area);
    
    0
}

