/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: frames_lf                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ This file is a wrapper for accessing the llfree library of Lars Wrenger.║
   ║                                                                         ║
   ║ Functions for saving free and reserved memory regions during booting:   ║
   ║   - boot_avail         insert free frame region detected during boot    ║
   ║   - boot_reserve       reserve a range of frames during boot            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 21.8.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::alloc::alloc_zeroed;
use core::alloc::Layout;
use log::info;
use x86_64::PhysAddr;
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::{PhysFrame, frame::PhysFrameRange};
use spin::Once;

use crate::memory::{PAGE_SIZE, dram};

use llfree::{Alloc, Init, LLFree, MetaData, Flags};

static PAGE_FRAME_ALLOCATOR: Once<LLFree> = Once::new();

/// Initialize the new page frame allocator.
pub fn init() {
    PAGE_FRAME_ALLOCATOR.call_once(|| {
        let cores = 1;
        let num_frames = dram::limit() as usize / PAGE_SIZE;

        info!("Initializing new page frame allocator with {} frames", num_frames);

        // Create meta data
        let m = LLFree::metadata_size(cores, num_frames);
        let local = aligned_buf(m.local);
        let trees = aligned_buf(m.trees);
        let meta = MetaData {
            local,
            trees,
            lower: aligned_buf(m.lower),
        };
        info!("MetaData:");
        info!("   local = {}", meta.local.len());
        info!("   trees = {}", meta.trees.len());
        info!("   lower = {}", meta.lower.len());

        // Create allocator for frames
        LLFree::new(cores, num_frames, Init::FreeAll, meta).unwrap()
    });
}

/// Check if the page frame allocator is currently locked.
pub fn allocator_locked() -> bool {
    false
}

/// Helper function to convert a u64 address to a PhysFrame.
/// The given address is aligned up to the page size (4 KiB).
pub fn frame_from_u64(addr: u64) -> Result<PhysFrame<Size4KiB>, x86_64::structures::paging::page::AddressNotAligned> {
    let pa = PhysAddr::new(addr).align_up(PAGE_SIZE as u64);
    PhysFrame::from_start_address(pa)
}

/// Allocate `frame_count` contiguous page frames.
/// The number of frames is rounded up to the next power of two!
pub fn alloc(frame_count: usize) -> PhysFrameRange {
    let rounded_frame_count = round_up_pow2(frame_count).unwrap();
    let exponent = rounded_frame_count.trailing_zeros(); // -> u32

    // Allocate 2^order frames
    // returning the offset in number of frames from beginning = 0
    match PAGE_FRAME_ALLOCATOR.get().unwrap().get(0, None, Flags::o(exponent as usize)) {
        Ok(first_frame) => {
            let start_frame = PhysFrame::containing_address(PhysAddr::new(first_frame as u64));

            let ret_frame_range = PhysFrameRange {
                start: start_frame,
                end:  start_frame + (PAGE_SIZE * rounded_frame_count) as u64,
            };
            info!("frames_lf::alloc frame_count={}, range = {:?}", frame_count, ret_frame_range);
            return ret_frame_range
        }
        Err(_) => {
            panic!("PageFrameAllocator: Out of memory!")
        }
    }
}

/// Free a contiguous range of page `frames`.
/// The number of frames must be a power of two otherwise the function will panic.
pub fn free(frames: PhysFrameRange) {

    let start_frame_number: u64 = frames.start.start_address().as_u64() / PAGE_SIZE as u64;
    let len = (frames.end.start_address().as_u64()
             - frames.start.start_address().as_u64())
             / PAGE_SIZE as u64;

    // must be power of two
    assert!(len.is_power_of_two());

    let exp = len.trailing_zeros() as usize;
    
    match PAGE_FRAME_ALLOCATOR.get().unwrap().put(0, start_frame_number as usize, Flags::o(exp)) {
        Ok(_first_frame) => {
            return ;
        }
        Err(_) => {
            panic!("PageFrameAllocator: free error!")
        }
    }
}




/// Marker for alignment, e.g., `#[repr(align(4096))] struct Align;`
#[repr(align(4096))]
struct AlignMarker;

pub fn aligned_buf(size: usize) -> &'static mut [u8] {
    let layout = Layout::from_size_align(size, align_of::<AlignMarker>()).unwrap();

    // SAFETY: caller must ensure allocator is initialized
    let ptr = unsafe { alloc_zeroed(layout) };

    if ptr.is_null() {
        panic!("Out of memory in aligned_buf!");
    }

    unsafe { core::slice::from_raw_parts_mut(ptr, size) }
}

/// Helper function for 'alloc'
pub fn round_up_pow2(n: usize) -> Option<usize> {
    if n == 0 {
        return None; // 0 cannot be rounded up to a power of two
    }
    let next = n.next_power_of_two();
    // If `n` was already the maximum possible power of two,
    // `next_power_of_two` will wrap to 0 in release or panic in debug.
    if next == 0 {
        None
    } else {
        Some(next)
    }
}
