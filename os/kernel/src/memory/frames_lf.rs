/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: frames_lf                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║                                                                         ║
   ║ Functions for saving free memory regions during booting:                ║
   ║   - add_free_region      add a free physical memory region              ║
   ║   - add_reserved_region  reserve a free physical memory region          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 24.5.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::sync::atomic::{AtomicUsize, AtomicU64, Ordering};
use log::info;
use spin::Mutex;
use x86_64::PhysAddr;
use x86_64::structures::paging::{frame::PhysFrameRange, PhysFrame};
use x86_64::structures::paging::Size4KiB;

use crate::memory::PAGE_SIZE;


/// Check if the page frame allocator is currently locked.
pub fn allocator_locked() -> bool {
    false
}


static PHYS_LIMIT: AtomicU64 = AtomicU64::new(0);

/// Get the highest physical address, managed by PAGE_FRAME_ALLOCATOR.
pub fn phys_limit() -> PhysFrame {
    let current_limit = PHYS_LIMIT.load(Ordering::SeqCst);
    PhysFrame::from_start_address(PhysAddr::new(current_limit))
        .expect("Physical limit is not aligned to page size")   
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



//
// From here data and code is only used during booting
//

// Storage for free memory regions inserted during booting
static MAX_FREE_REGIONS: usize = 1024;
static FREE_FRAME_REGIONS: Mutex<[u64; MAX_FREE_REGIONS]> = Mutex::new([0; MAX_FREE_REGIONS]);
static NEXT_FREE_FRAME_REGION: AtomicUsize = AtomicUsize::new(0);

// Insert a free frame region into the free frame region array
pub fn add_free_region(region: PhysFrameRange) {
    let mut free_start = region.start.start_address().as_u64();
    let mut free_end = region.end.start_address().as_u64();

    if free_start % PAGE_SIZE as u64 != 0 || free_end % PAGE_SIZE as u64 != 0 {
        panic!("Region not aligned to PAGE_SIZE");
    }
    if free_start >= free_end {
        panic!("Region free_start >= free_end");
    }


    // Make sure, the first page is not inserted to avoid null pointer panics
    if free_start == 0 {
        free_start = 0x1000;
    }

    // Update the physical limit if this region extends beyond the current limit
    let current_limit = PHYS_LIMIT.load(Ordering::SeqCst);
    if free_end > current_limit {
        PHYS_LIMIT.store(free_end, Ordering::SeqCst);
    }

    // Store the region in the free frame regions array
    let mut regions = FREE_FRAME_REGIONS.lock();
    let mut merged_start = free_start;
    let mut merged_end = free_end;

    let mut new_regions = [0u64; MAX_FREE_REGIONS];
    let mut new_index = 0;

    // Merge overlapping/adjacent regions into merged_start/end
    let current_len = NEXT_FREE_FRAME_REGION.load(Ordering::SeqCst);
    for i in (0..current_len).step_by(2) {
        let existing_start = regions[i];
        let existing_end = regions[i + 1];

        // Overlapping or adjacent
        if !(merged_end < existing_start || merged_start > existing_end) {
            merged_start = merged_start.min(existing_start);
            merged_end = merged_end.max(existing_end);
        } else {
            // Keep this region
            new_regions[new_index] = existing_start;
            new_regions[new_index + 1] = existing_end;
            new_index += 2;
        }
    }

    // Add the merged region
    if new_index + 2 > MAX_FREE_REGIONS {
        panic!("Too many regions");
    }
    new_regions[new_index] = merged_start;
    new_regions[new_index + 1] = merged_end;
    new_index += 2;

    // Copy back
    regions[..new_index].copy_from_slice(&new_regions[..new_index]);
    NEXT_FREE_FRAME_REGION.store(new_index, Ordering::SeqCst);
}


// Reserve a frame region 
pub fn add_reserved_region(region: PhysFrameRange) {
    let reserve_start = region.start.start_address().as_u64();
    let reserve_end = region.end.start_address().as_u64();

    if reserve_start % PAGE_SIZE as u64 != 0 || reserve_end % PAGE_SIZE as u64 != 0 {
        panic!("Reserved region is not page-aligned");
    }
    if reserve_start >= reserve_end {
        panic!("Reserved region start >= end");
    }

    let mut regions = FREE_FRAME_REGIONS.lock();
    let mut new_regions = [0u64; MAX_FREE_REGIONS];
    let mut new_index = 0;

    let current_len = NEXT_FREE_FRAME_REGION.load(Ordering::SeqCst);

    for i in (0..current_len).step_by(2) {
        let free_start = regions[i];
        let free_end = regions[i + 1];

        // No overlap
        if reserve_end <= free_start || reserve_start >= free_end {
            new_regions[new_index] = free_start;
            new_regions[new_index + 1] = free_end;
            new_index += 2;
            continue;
        }

        // Reserve overlaps part of the region — split if necessary
        if reserve_start > free_start {
            new_regions[new_index] = free_start;
            new_regions[new_index + 1] = reserve_start;
            new_index += 2;
        }

        if reserve_end < free_end {
            new_regions[new_index] = reserve_end;
            new_regions[new_index + 1] = free_end;
            new_index += 2;
        }
    }

    if new_index > MAX_FREE_REGIONS {
        panic!("Too many free regions after reservation — increase MAX_FREE_REGIONS");
    }

    // Copy back to the real region list
    regions[..new_index].copy_from_slice(&new_regions[..new_index]);
    NEXT_FREE_FRAME_REGION.store(new_index, Ordering::SeqCst);
}









// Dump the free frame regions to the log
pub fn dump() {
    info!("Free frame regions:");
    let mut regions = FREE_FRAME_REGIONS.lock();

    for i in (0..NEXT_FREE_FRAME_REGION.load(Ordering::SeqCst)).step_by(2) {
        info!("Region {}: Start: {:#x}, End:   {:#x}", i, regions[i], regions[i+1] );
    }
}

