/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: dram                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ This module provides functions for collecting information regarding     ║
   ║ available and reserved physical memory regions from EFI during boot     ║
   ║ time. After the kernel heap and page frame allocator are setup this     ║ 
   ║ module is no longer used.                                               ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - limit             highest dram address on this system               ║
   ║   - insert_available  insert a available dram region                    ║
   ║   - insert_reserved   insert a reserved dram region                     ║
   ║   - finalize          remove reserved regions from available regions    ║
   ║   - boot_alloc        alloc a region from available (only during boot)  ║
   ║   - dump              dump the collected dram information               ║
   ║   - get_all_reserved  get a ro view into the finalized reserved regions ║
   ║   - get_all_available get a ro view into the finalized avail. regions   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 2.4.2026                 ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::ops::DerefMut;
use core::sync::atomic::{AtomicU64, AtomicBool, Ordering};
use log::info;
use spin::{Mutex,MutexGuard};
use x86_64::PhysAddr;
use x86_64::structures::paging::{PhysFrame, frame::PhysFrameRange};

use crate::memory::PAGE_SIZE;


static DRAM_LIMIT: AtomicU64 = AtomicU64::new(0); // Highest physical address of the DRAM
static DRAM_FINALIZED: AtomicBool = AtomicBool::new(false);

/// Get the highest physical dram address + 1
pub fn limit() -> u64 {
    DRAM_LIMIT.load(Ordering::SeqCst)
}



const MAX_REGIONS: usize = 1024;

#[derive(Clone, Copy, Debug)]
pub struct Region {
    pub start: PhysAddr,
    pub end: PhysAddr, // exclusive
}

const EMPTY_REGION: Region = Region {
    start: PhysAddr::new(0),
    end: PhysAddr::new(0),
};

#[derive(Debug)]
struct RegionSet {
    count: usize,
    regions: [Region; MAX_REGIONS],
}

const EMPTY_REGION_SET: RegionSet = RegionSet {
    count: 0,
    regions: [EMPTY_REGION; MAX_REGIONS],
};

static AVAILABLE_REGIONS: Mutex<RegionSet> = Mutex::new(EMPTY_REGION_SET);
static RESERVED_REGIONS: Mutex<RegionSet> = Mutex::new(EMPTY_REGION_SET);


/// Insert a available physical memory region (retrieved from EFI) into the available region set
pub fn insert_available(region: PhysFrameRange) {
    if DRAM_FINALIZED.load(Ordering::Acquire) {
        panic!("available: DRAM regions have already been finalized");
    }
    
    let mut available = AVAILABLE_REGIONS.lock();

    // Make sure, the first page is not inserted to avoid null pointer panics
    if region.start.start_address().as_u64() == 0 {
        if region.end.start_address().as_u64() <= PAGE_SIZE as u64 {
            // This region is too small to be useful, skip it entirely
            return;
        }
        let mut region_modified = region;
        region_modified.start = PhysFrame::from_start_address(PhysAddr::new(0x1000)).expect("Invalid start address");
        insert_region(&mut *available, region_modified);
    } else {
        insert_region(&mut *available, region);
    }
}

/// Insert a reserved physical memory region (retrieved from EFI) or any other stuff identified by the  boot process
pub fn insert_reserved(region: PhysFrameRange) {
    if DRAM_FINALIZED.load(Ordering::Acquire) {
        panic!("reserved: DRAM regions have already been finalized");
    }
    let mut reserved = RESERVED_REGIONS.lock();
    insert_region(&mut *reserved, region);
}


/// Internal function for actual inserting a region into a RegionSet.
/// 
/// This is called both before and after finalization.
fn insert_region(set: &mut RegionSet, new_region: PhysFrameRange) {
    let start = new_region.start.start_address();
    let end = new_region.end.start_address(); // exclusive

    // Update the physical limit if this region extends beyond the current limit.
    DRAM_LIMIT.fetch_max(end.as_u64(), Ordering::SeqCst);

    let regions = &mut set.regions;
    let mut count = set.count;

    assert!(count <= MAX_REGIONS, "available: region count exceeds MAX_REGIONS");

    if count == MAX_REGIONS {
        panic!("available: too many regions");
    }

    // Find insertion point so the array remains sorted by start address.
    let mut insert_at = 0;
    while insert_at < count && regions[insert_at].start.as_u64() < start.as_u64() {
        insert_at += 1;
    }

    // Shift right to make room for the new region.
    for i in (insert_at..count).rev() {
        regions[i + 1] = regions[i];
    }

    regions[insert_at] = Region { start, end };
    count += 1;

    // Merge overlapping or adjacent regions in-place.
    //
    // For half-open regions [a, b) and [c, d), they overlap or touch iff c <= b.
    let mut write = 0;
    for read in 1..count {
        let current = regions[write];
        let next = regions[read];

        if next.start.as_u64() <= current.end.as_u64() {
            if next.end > regions[write].end {
                regions[write].end = next.end;
            }
        } else {
            write += 1;
            regions[write] = next;
        }
    }

    let new_count = if count == 0 { 0 } else { write + 1 };

    // Clear trailing slots for easier debugging.
    for slot in &mut regions[new_count..count] {
        *slot = EMPTY_REGION;
    }

    set.count = new_count;
}


/// Merge and update the available and reserved region sets after all regions have been inserted.
pub fn finalize() {
    finalize_available_regions();
    finalize_reserved_regions();
}

/// Remove all reserved regions from the available-region set.
/// After this function returns: available regions are sorted, merged, and non-overlapping
fn finalize_available_regions() {
    if DRAM_FINALIZED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        panic!("available: DRAM regions have already been finalized");
    }

    let mut available = AVAILABLE_REGIONS.lock();
    let reserved = RESERVED_REGIONS.lock();

    // Output buffer for the final available regions after subtracting reserved areas.
    let mut result = [EMPTY_REGION; MAX_REGIONS];
    let mut result_count = 0usize;

    // Helper to append a region to `result`.
    //
    // We only emit non-empty half-open regions [start, end).
    let mut push_region = |start: PhysAddr, end: PhysAddr| {
        if start.as_u64() >= end.as_u64() {
            return;
        }

        if result_count >= MAX_REGIONS {
            panic!("available: too many regions after finalization");
        }

        result[result_count] = Region { start, end };
        result_count += 1;
    };

    // For each available region, subtract every overlapping reserved region.
    let mut r_idx = 0usize;

    for f_idx in 0..available.count {
        let f = available.regions[f_idx];
        let f_start = f.start.as_u64();
        let f_end = f.end.as_u64();

        if f_start >= f_end {
            continue;
        }

        let mut cur_start = f.start;

        // Skip reserved regions that end before this available region starts.
        while r_idx < reserved.count
            && reserved.regions[r_idx].end.as_u64() <= f_start
        {
            r_idx += 1;
        }

        let mut scan = r_idx;

        // Process all reserved regions overlapping this available region.
        while scan < reserved.count {
            let r = reserved.regions[scan];
            let r_start = r.start.as_u64();
            let r_end = r.end.as_u64();

            // No more overlap with this available region.
            if r_start >= f_end {
                break;
            }

            // Emit the gap before the reserved region, if any.
            if cur_start.as_u64() < r_start {
                let gap_end = if r.start < f.end { r.start } else { f.end };
                push_region(cur_start, gap_end);
            }

            // Advance cur_start beyond the reserved region if it overlaps.
            if r_end > cur_start.as_u64() {
                let new_start = if r.end > f.end { f.end } else { r.end };
                cur_start = new_start;
            }

            // This available region is fully consumed.
            if cur_start.as_u64() >= f_end {
                break;
            }

            scan += 1;
        }

        // Emit the tail after the last overlapping reserved region.
        if cur_start.as_u64() < f_end {
            push_region(cur_start, f.end);
        }
    }

    // Store final available regions.
    available.count = result_count;
    for i in 0..result_count {
        available.regions[i] = result[i];
    }
    for i in result_count..MAX_REGIONS {
        available.regions[i] = EMPTY_REGION;
    }
}


/// Rebuild RESERVED_REGIONS as the complement of AVAILABLE_REGIONS in [0, DRAM_LIMIT).
fn finalize_reserved_regions() {
    if !DRAM_FINALIZED.load(Ordering::Acquire) {
        panic!("reserved: DRAM not finalized yet");
    }

    let dram_limit = DRAM_LIMIT.load(Ordering::Acquire);
    let available = AVAILABLE_REGIONS.lock();
    let mut reserved = RESERVED_REGIONS.lock();

    assert!(
        available.count <= MAX_REGIONS,
        "reserved: available region count exceeds MAX_REGIONS"
    );

    // Clear old reserved regions completely.
    reserved.count = 0;
    for slot in &mut reserved.regions {
        *slot = EMPTY_REGION;
    }

    if dram_limit == 0 {
        return;
    }

    let mut cur = PhysAddr::new(0);

    for i in 0..available.count {
        let region = available.regions[i];

        let start = region.start.as_u64();
        let end = region.end.as_u64();

        debug_assert!(start <= end);
        debug_assert!(end <= dram_limit);

        // Gap before this available region => reserved.
        if cur.as_u64() < start {
            if reserved.count >= MAX_REGIONS {
                panic!("reserved: too many regions after finalization");
            }

            let idx = reserved.count;
            reserved.regions[idx] = Region {
                start: cur,
                end: region.start,
            };
            reserved.count = idx + 1;
        }

        if end > cur.as_u64() {
            cur = region.end;
        }
    }

    // Tail gap from the end of the last available region to DRAM_LIMIT.
    if cur.as_u64() < dram_limit {
        if reserved.count >= MAX_REGIONS {
            panic!("reserved: too many regions after finalization");
        }

        let idx = reserved.count;
        reserved.regions[idx] = Region {
            start: cur,
            end: PhysAddr::new(dram_limit),
        };
        reserved.count = idx + 1;
    }
}

/// Allocate `num_frames` contiguous page frames from the first fitting region.
/// This function is only used to allocate the kernel heap during booting. \
/// This is necessary because the page frame allocator needs a heap for its initialization to store its metadata. \
/// So we need a kernel heap first.
///
/// Returns a half-open physical frame range: [start, end) on success, or `None` if no fitting region is found.
/// 
/// This function can only be called after finalization.
pub fn boot_alloc(num_frames: usize) -> Option<PhysFrameRange> {
    assert!(num_frames > 0, "alloc: num_frames must be > 0");
    if !DRAM_FINALIZED.load(Ordering::Acquire) {
        panic!("boot_alloc: DRAM not finalized yet");
    }

    let byte_len = num_frames
        .checked_mul(PAGE_SIZE)
        .expect("alloc: frame count overflow");

    let byte_len = u64::try_from(byte_len)
        .expect("alloc: allocation size does not fit into u64");

    let mut available = AVAILABLE_REGIONS.lock();
    let count = available.count;

    assert!(count <= MAX_REGIONS, "alloc: available region count exceeds MAX_REGIONS");

    for i in 0..count {
        let region = available.regions[i];

        let start = region.start.as_u64();
        let end = region.end.as_u64();

        debug_assert!(start % PAGE_SIZE as u64 == 0);
        debug_assert!(end % PAGE_SIZE as u64 == 0);
        debug_assert!(start < end);

        let region_size = end - start;
        if region_size < byte_len {
            continue;
        }

        let alloc_start = region.start;
        let alloc_end = PhysAddr::new(
            start
                .checked_add(byte_len)
                .expect("alloc: physical address overflow"),
        );

        let start_frame = PhysFrame::from_start_address(alloc_start)
            .expect("alloc: invalid start frame address");
        let end_frame = PhysFrame::from_start_address(alloc_end)
            .expect("alloc: invalid end frame address");

        if alloc_end == region.end {
            // Entire region consumed: remove it and keep remaining regions sorted.
            let last = available.count - 1;
            for j in i..last {
                available.regions[j] = available.regions[j + 1];
            }
            available.regions[last] = EMPTY_REGION;
            available.count = last;
        } else {
            // Shrink region from the front.
            available.regions[i].start = alloc_end;
        }

        let allocated_range = PhysFrameRange {
            start: start_frame,
            end: end_frame,
        };
        // add this to the reserved regions, so that the page frame allocator will not overwrite this
        insert_region(RESERVED_REGIONS.lock().deref_mut(), allocated_range);
        return Some(allocated_range);
    }

    None
}


/// Dump the DRAM region state to the log.
pub fn dump() {
    info!("DRAM information");
    info!("   limit: {:#x}", DRAM_LIMIT.load(Ordering::SeqCst));

    info!("   available frame regions:");
    {
        let available = AVAILABLE_REGIONS.lock();

        assert!(
            available.count <= MAX_REGIONS,
            "dump: available region count exceeds MAX_REGIONS"
        );

        for i in 0..available.count {
            let region = available.regions[i];
            let start = region.start.as_u64();
            let end = region.end.as_u64();
            let num_frames = (end - start) / PAGE_SIZE as u64;

            info!(
                "      [{:#10x} - {:#10x}), #frames: [{}]",
                start,
                end,
                num_frames
            );
        }
    }

    info!("   reserved frame regions:");
    {
        let reserved = RESERVED_REGIONS.lock();

        assert!(
            reserved.count <= MAX_REGIONS,
            "dump: reserved region count exceeds MAX_REGIONS"
        );

        for i in 0..reserved.count {
            let region = reserved.regions[i];
            let start = region.start.as_u64();
            let end = region.end.as_u64();
            let num_frames = (end - start) / PAGE_SIZE as u64;

            info!(
                "      [{:#10x} - {:#10x}), #frames: [{}]",
                start,
                end,
                num_frames
            );
        }
    }
}


/// Read-only view into the finalized region
/// Used to initialize the page frame allocator with the collected  regions 
pub struct RegionsGuard<'a> {
    guard: MutexGuard<'a, RegionSet>,
}

impl<'a> RegionsGuard<'a> {
    /// Number of regions
    pub fn count(&self) -> usize {
        self.guard.count
    }

    /// Get a region by index
    pub fn get(&self, index: usize) -> Option<Region> {
        if index < self.guard.count {
            Some(self.guard.regions[index])
        } else {
            None
        }
    }

    /// Iterate over all regions
    pub fn iter(&self) -> impl Iterator<Item = &Region> {
        self.guard.regions[..self.guard.count].iter()
    }

    /// Direct slice access (still read-only)
    pub fn as_slice(&self) -> &[Region] {
        &self.guard.regions[..self.guard.count]
    }
}


/// Get a read-only view into the finalized reserved regions
pub fn get_all_reserved<'a>() -> RegionsGuard<'a> {
    if !DRAM_FINALIZED.load(Ordering::Acquire) {
        panic!("available: DRAM not finalized yet");
    }

    RegionsGuard {
        guard: RESERVED_REGIONS.lock(),
    }
}

/// Get a read-only view into the finalized available regions
pub fn get_all_available<'a>() -> RegionsGuard<'a> {
    if !DRAM_FINALIZED.load(Ordering::Acquire) {
        panic!("available: DRAM not finalized yet");
    }

    RegionsGuard {
        guard: AVAILABLE_REGIONS.lock(),
    }
}