/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_vmem                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to virtual memory management.          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 30.8.2024, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use crate::memory::vmm::{VirtualMemoryArea, VmaType};
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process_manager;
use x86_64::structures::paging::PageTableFlags;


pub fn sys_map_user_heap(size: usize) -> isize {
    let process = process_manager().read().current_process();
    let code_areas = process.find_vmas(VmaType::Code);
    let code_area = code_areas.get(0).expect("Process does not have code area!");
    let heap_start = code_area.end().align_up(PAGE_SIZE as u64);
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);

    process.address_space().map(heap_area.range(), MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
    process.add_vma(heap_area);

    heap_start.as_u64() as isize
}

