/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: frames_lf                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║                                                                         ║
   ║ Functions for saving free and reserved memory regions during booting:   ║
   ║   - boot_avail         insert free frame region detected during boot    ║
   ║   - boot_reserve       reserve a range of frames during boot            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 22.7.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use log::info;
use x86_64::PhysAddr;
use x86_64::structures::paging::{frame::PhysFrameRange, PhysFrame};
use x86_64::structures::paging::Size4KiB;

use crate::memory::PAGE_SIZE;


/// Check if the page frame allocator is currently locked.
pub fn allocator_locked() -> bool {
    false
}


/// Helper function to convert a u64 address to a PhysFrame.
/// The given address is aligned up to the page size (4 KiB).
pub fn frame_from_u64(
    addr: u64,
) -> Result<PhysFrame<Size4KiB>, x86_64::structures::paging::page::AddressNotAligned> {
    let pa = PhysAddr::new(addr).align_up(PAGE_SIZE as u64);
    PhysFrame::from_start_address(pa)
}

/// Allocate `frame_count` contiguous page frames.
pub fn alloc(frame_count: usize) -> PhysFrameRange {
//    PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count)
   panic!("Page frame allocator is not implemented yet"); 
   PhysFrameRange {
        start: PhysFrame::from_start_address(PhysAddr::new(0x1000)).expect("Invalid start address"),
        end: PhysFrame::from_start_address(PhysAddr::new(0x1000 + frame_count as u64 * PAGE_SIZE as u64))
            .expect("Invalid end address"),
    }
}

/// Free a contiguous range of page `frames`.
/// Unsafe because invalid parameters may break the list allocator.
pub unsafe fn free(frames: PhysFrameRange) {
   panic!("Page frame allocator is not implemented yet"); 
/*    unsafe {
        PAGE_FRAME_ALLOCATOR.lock().free_block(frames);
    }
    */
}

/*
/// Get a dump of the current free list.
pub fn dump() -> String {
    "TEST"
 //   format!("{:?}", PAGE_FRAME_ALLOCATOR.lock())
}
*/


