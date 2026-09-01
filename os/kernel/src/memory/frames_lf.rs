/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: frames_lf                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ This file is a wrapper for accessing the llfree library of Lars Wrenger.║
   ║ Currently, it is configured for single core usage.                      ║
   ║                                                                         ║
   ║ Functions for saving free and reserved memory regions during booting:   ║
   ║   - allocator_locked   check if the allocator is currently locked       ║
   ║   - init               insert free frame region detected during boot    ║
   ║   - alloc              allocate a range of frames during runtime        ║
   ║   - free               free a range of frames during runtime            ║
   ║   - dump               reserve a range of frames during boot            ║
   ║   - frame_from_u64     convert a u64 address to a PhysFrame             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 01.4.2026                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::alloc::alloc_zeroed;
use alloc::boxed::Box;
use core::alloc::Layout;
use log::{info,debug};
use spin::Once;
use x86_64::PhysAddr;
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::{PhysFrame, frame::PhysFrameRange};
use llfree::{Alloc, FrameId, Init, LLFree, MetaData, Request, Tiering, MAX_ORDER};

use crate::memory::{PAGE_SIZE, dram};


/// params: (order, core_nr) -> Request
type RequestFn = dyn Fn(usize, usize) -> Request + Send + Sync + 'static;

static PAGE_FRAME_ALLOCATOR: Once<(LLFree, Box<RequestFn>)> = Once::new();

const CORE_NR: usize = 0; // to be replaced later 

/// Initialize the new page frame allocator and remove reserved regions 
pub fn init() {
    PAGE_FRAME_ALLOCATOR.call_once(|| {
        let cores = 1;
        let num_frames = dram::limit() as usize / PAGE_SIZE;

        debug!("Initializing new page frame allocator with {} frames for {} cores", num_frames, cores);

        // 'tiering':
        //    Tiering describes the allocator layout (tiers, sizes, structure)
        //    Here we use 'simple' policy => two tiers, `0` for small and `1` for huge frames, and local reservations for each core.
        // 'request':
        //    Translates allocation parameters into an internal tiered allocation model.
        //    Here params are (order, core_nr)
        let (tiering, request) = Tiering::simple(1);

        // Create meta data required for the allocator
        let m = LLFree::metadata_size(&tiering, num_frames);
        let local = aligned_buf(m.local);
        let trees = aligned_buf(m.trees);
        let meta = MetaData {
            local,
            trees,
            lower: aligned_buf(m.lower),
        };
        debug!("MetaData:");
        debug!("   local = {}", meta.local.len());
        debug!("   trees = {}", meta.trees.len());
        debug!("   lower = {}", meta.lower.len());

        // Create allocator for frames
        (LLFree::new(num_frames, Init::FreeAll, &tiering, meta).unwrap(), Box::new(request))
    });

    // remove reserved regions from the allocator so they are not available for allocation later
    finalize(); 
}

pub fn dump() {
    let v = PAGE_FRAME_ALLOCATOR.get().unwrap();
    let s = v.0.stats();

    info!("LLFree Stats: {:?}", s);
}

pub fn get_total_free_frames() -> usize {
    1
}

/// Check if the page frame allocator is currently locked.
pub fn allocator_locked() -> bool {
    false
}

/// Remove reserved memory regions
fn finalize() {
    let free = dram::get_all_reserved();
    for r in free.iter() {
        let start_addr = r.start.as_u64();
        let len = r.end.as_u64().saturating_sub(start_addr);

        if len == 0 {
            continue;
        }

        debug_assert_eq!(start_addr % PAGE_SIZE as u64, 0);
        debug_assert_eq!(r.end.as_u64() % PAGE_SIZE as u64, 0);

        let frame_count = (len / PAGE_SIZE as u64) as usize;
        mark_reserved(start_addr, frame_count);
    }
}


/// Allocate `frame_count` contiguous page frames.
/// The number of frames is rounded up to the next power of two!
pub fn alloc(frame_count: usize) -> PhysFrameRange {
    alloc_internal(None, frame_count)
}


/// Mark `frame_count` contiguous page frames, starting from `at_phys_addr`, as reserved (not free).
/// The starting physical address must be aligned to the page size (4 KiB).
/// Only used during booting to reserve memory regions that should not be allocated later.
fn mark_reserved(at_phys_addr: u64, frame_count: usize) {
    assert!(
        at_phys_addr % PAGE_SIZE as u64 == 0,
        "Starting physical address must be aligned to page size (4 KiB)"
    );
    assert!(frame_count > 0, "frame_count must be greater than zero");

    let mut remaining = frame_count;
    let mut current_addr = at_phys_addr;

    let max_chunk = 1usize << MAX_ORDER;

    while remaining > 0 {
        let current_frame = current_addr as usize / PAGE_SIZE;

        // Largest power of two <= remaining, but never larger than MAX_ORDER.
        let mut chunk = if remaining >= max_chunk {
            max_chunk
        } else {
            1usize << (usize::BITS - 1 - remaining.leading_zeros())
        };

        // Reduce chunk until the current frame is aligned for it.
        while current_frame % chunk != 0 {
            chunk >>= 1;
        }

        alloc_internal(Some(FrameId(current_frame)), chunk);

        current_addr += (chunk * PAGE_SIZE) as u64;
        remaining -= chunk;
    }
}


/// Allocate `frame_count` contiguous page frames starting from given  `frame_number` (if provided) or from any free frame is `None`.
/// The number of frames is rounded up to the next power of two!
fn alloc_internal(frame_start: Option<FrameId>, frame_count: usize) -> PhysFrameRange {
    let rounded_frame_count = round_up_pow2(frame_count).unwrap();
    let order = rounded_frame_count.trailing_zeros() as usize;
    let alloc = PAGE_FRAME_ALLOCATOR.get().expect("page frame allocator not initialized");

    // Allocate 2^order frames
    // returning the offset in number of frames from beginning = 0
    match alloc.0.get(frame_start, (alloc.1)(order, CORE_NR)) {
        Ok((frame_id, _tier)) => {
            let start_addr = PhysAddr::new(frame_id.into_bits() as u64 * PAGE_SIZE as u64);
            let start_frame = PhysFrame::from_start_address(start_addr).expect("llfree returned unaligned frame address");

            let ret_frame_range: PhysFrameRange = PhysFrameRange {
                start: start_frame,
                end: start_frame + rounded_frame_count as u64,
            };

            ret_frame_range
        }
        Err(e) => panic!("PageFrameAllocator: error {:?}", e),
    }
}

/// Free a contiguous range of page `frames`.
/// The number of frames must be a power of two otherwise the function will panic.
pub fn free(frames: PhysFrameRange) {
    let alloc = PAGE_FRAME_ALLOCATOR.get().expect("page frame allocator not initialized");

    let start_frame_number = frames.start.start_address().as_u64() / PAGE_SIZE as u64;
    let nr_of_frames = (frames.end.start_address().as_u64() - frames.start.start_address().as_u64()) / PAGE_SIZE as u64;

    assert!(nr_of_frames.is_power_of_two(), "page frame range length must be a power of two");

    let order = nr_of_frames.trailing_zeros();

    match alloc.0.put(FrameId(start_frame_number as usize), (alloc.1)(order as usize, CORE_NR)) {
        Ok(_first_frame) => {}
        Err(_) => panic!("PageFrameAllocator: free error!"),
    }
}

/// Marker for alignment, e.g., `#[repr(align(4096))] struct Align;`
#[repr(align(4096))]
struct AlignMarker;

fn aligned_buf(size: usize) -> &'static mut [u8] {
    let layout = Layout::from_size_align(size, align_of::<AlignMarker>()).unwrap();

    // SAFETY: caller must ensure allocator is initialized
    let ptr = unsafe { alloc_zeroed(layout) };

    if ptr.is_null() {
        panic!("Out of memory in aligned_buf!");
    }

    unsafe { core::slice::from_raw_parts_mut(ptr, size) }
}

/// Helper function for 'alloc'
fn round_up_pow2(n: usize) -> Option<usize> {
    if n == 0 {
        return None; // 0 cannot be rounded up to a power of two
    }
    let next = n.next_power_of_two();
    // If `n` was already the maximum possible power of two,
    // `next_power_of_two` will wrap to 0 in release or panic in debug.
    if next == 0 { None } else { Some(next) }
}

/// Helper function to convert a u64 address to a PhysFrame.
/// The given address is aligned up to the page size (4 KiB).
pub(super) fn frame_from_u64(
    addr: u64,
) -> Result<PhysFrame<Size4KiB>, x86_64::structures::paging::page::AddressNotAligned> {
    let pa = PhysAddr::new(addr).align_up(PAGE_SIZE as u64);
    PhysFrame::from_start_address(pa)
}