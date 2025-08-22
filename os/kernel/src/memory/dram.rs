/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: dram                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ This module provides functions for collecting information regarding free║
   ║ and reserved frame regions from EFI during boot time. After the kernel  ║
   ║ heap and page frame allocator are setup this module is no longer used.  ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - limit         highest dram address on this system                   ║
   ║   - available     insert free frame region                              ║
   ║   - reserved      remove a reserved frame range from available          ║
   ║   - alloc         alloc a range of frames from avail. during boot       ║
   ║   - finalize      merge all reserved r. into free regions               ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 21.8.2025                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use log::info;
use spin::Mutex;
use x86_64::PhysAddr;
use x86_64::structures::paging::{PhysFrame, frame::PhysFrameRange};

use crate::memory::PAGE_SIZE;
use crate::memory::frames::phys_limit;

static DRAM_LIMIT: AtomicU64 = AtomicU64::new(0); // Highest physical address of the DRAM
static DRAM_FINALIZED: AtomicU64 = AtomicU64::new(0); // Flag to indicate if the DRAM regions have been finalized

/// Get the highest physical dram address + 1
pub fn limit() -> u64 {
    DRAM_LIMIT.load(Ordering::SeqCst)
}

static MAX_REGIONS: usize = 1024;

// Storage for free memory regions collected during booting
static FREE_FRAME_REGIONS: Mutex<[u64; MAX_REGIONS]> = Mutex::new([0; MAX_REGIONS]);
static NEXT_FREE_FRAME_INDEX: AtomicUsize = AtomicUsize::new(0);

// Storage for reserved memory regions collected during booting
static RESERVED_FRAME_REGIONS: Mutex<[u64; MAX_REGIONS]> = Mutex::new([0; MAX_REGIONS]);
static NEXT_FREE_RESERVED_FRAME_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Allocate a free frame region from free frame region array. This only used to allocate the kernel heap during booting. \
/// This is necessary because the page frame allocator needs a heap for its initialization to store its metadata.
pub fn alloc(num_frames: u64) -> Option<PhysFrameRange> {
    if DRAM_FINALIZED.load(Ordering::SeqCst) != 0 {
        panic!("DRAM regions have already been finalized, cannot allocate frames");
    }

    let mut free_regions = FREE_FRAME_REGIONS.lock();

    // Search for a free region that is large enough
    let current_len = NEXT_FREE_FRAME_INDEX.load(Ordering::SeqCst);
    for i in (0..current_len).step_by(2) {
        let free_start = free_regions[i];
        let free_end = free_regions[i + 1];

        // Check if the region is large enough
        if (free_end - free_start) / PAGE_SIZE as u64 >= num_frames {
            // Found a region that is large enough
            let start = PhysFrame::from_start_address(PhysAddr::new(free_start)).expect("Invalid start address");
            let end = PhysFrame::from_start_address(PhysAddr::new(free_start + num_frames * PAGE_SIZE as u64)).expect("Invalid end address");

            // Update the free region
            free_regions[i] = free_start + num_frames * PAGE_SIZE as u64;
            if free_regions[i] >= free_regions[i + 1] {
                // Remove the region if it is empty now
                free_regions[i] = 0;
                free_regions[i + 1] = 0;
            }

            return Some(PhysFrameRange { start, end });
        }
    }
    None
}

/// Insert a free frame region (retrieved from EFI) into the free frame region array
pub fn available(new_region: PhysFrameRange) {
    if DRAM_FINALIZED.load(Ordering::SeqCst) != 0 {
        panic!("DRAM regions have already been finalized, cannot insert available frames");
    }

    let mut new_region_start = new_region.start.start_address().as_u64();
    let new_region_end = new_region.end.start_address().as_u64();

    if new_region_start % PAGE_SIZE as u64 != 0 || new_region_end % PAGE_SIZE as u64 != 0 {
        panic!("Region not aligned to PAGE_SIZE");
    }
    if new_region_start >= new_region_end {
        panic!("Region free_start >= free_end");
    }

    // Make sure, the first page is not inserted to avoid null pointer panics
    if new_region_start == 0 {
        new_region_start = 0x1000;
    }

    // Update the physical limit if this region extends beyond the current limit
    let current_limit = DRAM_LIMIT.load(Ordering::SeqCst);
    if new_region_end > current_limit {
        DRAM_LIMIT.store(new_region_end, Ordering::SeqCst);
    }

    // Store the region in the free frame regions array
    let mut merged_start = new_region_start;
    let mut merged_end = new_region_end;

    let mut new_regions = [0u64; MAX_REGIONS];
    let mut new_index = 0;

    // Merge overlapping/adjacent regions into merged_start/end
    let mut free_regions = FREE_FRAME_REGIONS.lock();
    let current_len = NEXT_FREE_FRAME_INDEX.load(Ordering::SeqCst);
    for i in (0..current_len).step_by(2) {
        let existing_start = free_regions[i];
        let existing_end = free_regions[i + 1];

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
    if new_index + 2 > MAX_REGIONS {
        panic!("Too many free regions");
    }
    new_regions[new_index] = merged_start;
    new_regions[new_index + 1] = merged_end;
    new_index += 2;

    // Copy back
    free_regions[..new_index].copy_from_slice(&new_regions[..new_index]);
    NEXT_FREE_FRAME_INDEX.store(new_index, Ordering::SeqCst);
}

/// Insert a reserved frame region (retrieved from EFI) into the reserved frame region array
pub fn reserved(reserve_region: PhysFrameRange) {
    if DRAM_FINALIZED.load(Ordering::SeqCst) != 0 {
        panic!("DRAM regions have already been finalized, cannot insert reserved frames");
    }

    let reserve_region_start = reserve_region.start.start_address().as_u64();
    let reserve_region_end = reserve_region.end.start_address().as_u64();

    if reserve_region_start % PAGE_SIZE as u64 != 0 || reserve_region_end % PAGE_SIZE as u64 != 0 {
        panic!("Region not aligned to PAGE_SIZE");
    }
    if reserve_region_start >= reserve_region_end {
        panic!("Region free_start >= free_end");
    }

    // Update the physical limit if this region extends beyond the current limit
    let current_limit = DRAM_LIMIT.load(Ordering::SeqCst);
    if reserve_region_end > current_limit {
        DRAM_LIMIT.store(reserve_region_end, Ordering::SeqCst);
    }

    // Store the region in the reserved frame regions array
    let mut merged_start = reserve_region_start;
    let mut merged_end = reserve_region_end;

    let mut new_regions = [0u64; MAX_REGIONS];
    let mut new_index = 0;

    // Merge overlapping/adjacent regions into merged_start/end
    let mut reserved_regions = RESERVED_FRAME_REGIONS.lock();
    let current_len = NEXT_FREE_RESERVED_FRAME_INDEX.load(Ordering::SeqCst);
    for i in (0..current_len).step_by(2) {
        let existing_start = reserved_regions[i];
        let existing_end = reserved_regions[i + 1];

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
    if new_index + 2 > MAX_REGIONS {
        panic!("Too many reerved regions");
    }
    new_regions[new_index] = merged_start;
    new_regions[new_index + 1] = merged_end;
    new_index += 2;

    // Copy back
    reserved_regions[..new_index].copy_from_slice(&new_regions[..new_index]);
    NEXT_FREE_RESERVED_FRAME_INDEX.store(new_index, Ordering::SeqCst);
}

/// Merge all reserved frame regions into the free frame regions.
/// After the first step the reserved frame regions are updated .
///
/// Note: After this function is called only `dram_limit` and `dram_dump` can be called.
/// Calling any other function will panic.
pub fn finalize() {
    {
        let reserved_regions = RESERVED_FRAME_REGIONS.lock();
        let current_len = NEXT_FREE_RESERVED_FRAME_INDEX.load(Ordering::SeqCst);

        for i in (0..current_len).step_by(2) {
            let reserve_start = reserved_regions[i];
            let reserve_end = reserved_regions[i + 1];

            if reserve_start == 0 && reserve_end == 0 {
                continue; // Skip empty regions
            }

            let reserve_region = PhysFrameRange {
                start: PhysFrame::from_start_address(PhysAddr::new(reserve_start)).expect("Invalid start address"),
                end: PhysFrame::from_start_address(PhysAddr::new(reserve_end)).expect("Invalid end address"),
            };

            merge_reserved_region(reserve_region);
        }
    } // lock released

    update_reserved_regions(phys_limit().start_address().as_u64());

    DRAM_FINALIZED.store(1, Ordering::SeqCst);
}

/// Merge all reserved frame regions with the free frame regions.
fn merge_reserved_region(reserve_region: PhysFrameRange) {
    let reserve_start = reserve_region.start.start_address().as_u64();
    let reserve_end = reserve_region.end.start_address().as_u64();

    if reserve_start % PAGE_SIZE as u64 != 0 || reserve_end % PAGE_SIZE as u64 != 0 {
        panic!("Reserved region is not page-aligned");
    }
    if reserve_start >= reserve_end {
        panic!("Reserved region start >= end");
    }

    let mut free_regions = FREE_FRAME_REGIONS.lock();
    let mut new_regions = [0u64; MAX_REGIONS];
    let mut new_index = 0;

    let current_len = NEXT_FREE_FRAME_INDEX.load(Ordering::SeqCst);

    for i in (0..current_len).step_by(2) {
        let free_start = free_regions[i];
        let free_end = free_regions[i + 1];

        // No overlap
        if reserve_end <= free_start || reserve_start >= free_end {
            new_regions[new_index] = free_start;
            new_regions[new_index + 1] = free_end;
            new_index += 2;
            continue;
        }

        // Reserve overlaps part of the region — split if necessary
        if reserve_start >= free_start {
            new_regions[new_index] = free_start;
            new_regions[new_index + 1] = reserve_start;
            new_index += 2;
        }

        if reserve_end <= free_end {
            new_regions[new_index] = reserve_end;
            new_regions[new_index + 1] = free_end;
            new_index += 2;
        }
    }

    if new_index > MAX_REGIONS {
        panic!("Too many regions — increase MAX_REGIONS");
    }

    // Copy back to the real region list
    free_regions[..new_index].copy_from_slice(&new_regions[..new_index]);
    NEXT_FREE_FRAME_INDEX.store(new_index, Ordering::SeqCst);
}

/// Update the reserved frame regions by inspecting all gaps in the frame regions
fn update_reserved_regions(phys_max: u64) {
    let free = FREE_FRAME_REGIONS.lock();
    
    let mut reserved = RESERVED_FRAME_REGIONS.lock();
    let mut res_index = 0;

    let mut cursor: u64 = 0;

    for i in (0..NEXT_FREE_FRAME_INDEX.load(Ordering::SeqCst)).step_by(2) {
        let free_start = free[i];
        let free_end = free[i + 1];
        if cursor < free_start {
            // gap between last reserved cursor and this free region
            if res_index < reserved.len() {
                reserved[res_index] = cursor;
                reserved[res_index + 1] = free_start;
                res_index += 2;
            }
        }

        // advance cursor to end of this free region
        cursor = free_end;
    }

    // after last free region → possible reserved tail up to phys_max
    if cursor < phys_max && res_index < reserved.len() {
        reserved[res_index] = cursor;
        reserved[res_index + 1] = phys_max;
        res_index += 1;
    }

    NEXT_FREE_RESERVED_FRAME_INDEX.store(res_index, Ordering::SeqCst);
}

/// Dump the free frame regions to the log
pub fn dump() {
    info!("DRAM information");
    info!("   limit: {:#x}", DRAM_LIMIT.load(Ordering::SeqCst));
    info!("   free frame regions:");
    let regions = FREE_FRAME_REGIONS.lock();
    for i in (0..NEXT_FREE_FRAME_INDEX.load(Ordering::SeqCst)).step_by(2) {
        let num_frames = (regions[i + 1] - regions[i]) / PAGE_SIZE as u64;
        info!("      [{:#10x} - {:#10x}], #frames: [{:?}]", regions[i], regions[i + 1], num_frames);
    }
    info!("  reserved frame regions:");
    let regions = RESERVED_FRAME_REGIONS.lock();
    for i in (0..NEXT_FREE_RESERVED_FRAME_INDEX.load(Ordering::SeqCst)).step_by(2) {
        let num_frames = (regions[i + 1] - regions[i]) / PAGE_SIZE as u64;
        info!("      [{:#10x} - {:#10x}], #frames: [{:?}]", regions[i], regions[i + 1], num_frames);
    }
}
