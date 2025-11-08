/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_vmem                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to virtual memory management.          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 24.5.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::sync::Arc;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Page, PageTableFlags};

use crate::memory::vma::VmaType;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process_manager;
use syscall::return_vals::Errno;
use mm::MmapFlags;

/// Map memory to a process.
///
/// This just sets up the VMA, no page tables are created yet.
/// This happens later on on page faults.
/// 
/// supports populating (aka fault in directly), which reduces overhead for larger buffers
pub fn sys_map_memory(start: usize, size: usize, options: usize) -> isize {
    let process = process_manager().read().current_process();

    let m_flags = MmapFlags::from_bits_truncate(options as u8);

    let start_page = if m_flags.contains(MmapFlags::ALLOC_AT) {
        let start_addr = VirtAddr::new(start.try_into().unwrap());
        Some(Page::containing_address(start_addr))
    } else {
        None
    };

    let num_pages = size.div_ceil(PAGE_SIZE);

    let (vma_type, vma_tag) = if m_flags.contains(MmapFlags::ANONYMOUS) {
        (VmaType::Anonymous, "anon")
    } else {
        (VmaType::Heap, "heap")
    };

    let vma = process.virtual_address_space.alloc_vma(
        start_page,
        num_pages as u64,
        MemorySpace::User,
        vma_type,
        vma_tag,
    );

    let Some(vma_u) = vma else {
        return Errno::EUNKN as isize;
    };

    if m_flags.contains(MmapFlags::POPULATE) {
        let complete_range = vma_u.range();
        process.virtual_address_space.map_partial_vma(
            &vma_u, 
            complete_range, 
            MemorySpace::User, 
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
    }

    vma_u.start().as_u64() as isize
}
