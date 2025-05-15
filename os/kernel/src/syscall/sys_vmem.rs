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
use log::{LevelFilter, debug, info, warn};


pub fn sys_map_user_heap(size: usize) -> isize {
    let process = process_manager().read().current_process();
    let code_areas = process.virtual_address_space.find_vmas(VmaType::Code);
    let highest_code_area = code_areas.iter()
        .max_by(|area1, area2| area1.end().as_u64().cmp(&area2.end().as_u64()))
        .unwrap();
    let heap_start = highest_code_area.end().align_up(PAGE_SIZE as u64);
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);
    
    info!("sys_map_user_heap");
    let l = crate::memory::frames::phys_limit();
    info!("phys_limit: {:?}", l);
    process.virtual_address_space.dump(process.id());



    process.virtual_address_space.map(heap_area.range(), MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE, VmaType::Heap, "");
    heap_start.as_u64() as isize
}

