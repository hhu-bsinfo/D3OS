pub mod vmm;
pub mod vma;
pub mod pages;

#[cfg(feature = "frame_alloc_locking")]
pub mod frames;

#[cfg(feature = "frame_alloc_lockfree")]
pub mod frames_lf;

pub mod nvmem;
pub mod dram;
pub mod shm;

pub mod heap;
pub mod stack;
pub mod acpi_handler;

use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::PhysAddr;
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::frame::PhysFrameRange;

#[cfg(feature = "frame_alloc_lockfree")]
use self::frames_lf as frames_impl;

#[cfg(feature = "frame_alloc_locking")]
use self::frames as frames_impl;

#[derive(PartialEq)]
#[derive(Clone, Copy, Debug)]
pub enum MemorySpace {
    Kernel,
    User
}

pub const PAGE_SIZE: usize = 0x1000;


static FREE_FRAMES: AtomicUsize = AtomicUsize::new(0);                   

pub fn init_total_free_frames() {
    FREE_FRAMES.store(get_total_free_frames(), Ordering::SeqCst);
}

pub fn get_free_frames() -> usize {
    FREE_FRAMES.load(Ordering::SeqCst)
}


// ### Wrapper functions for the page frame allocator in `frames.rs` or `frames_lf.rs` (news lockfree implementation) ###

/// Wrapper function
pub fn init() {
    frames_impl::init();
}

/// Wrapper function
pub fn dump() {
    frames_impl::dump();
}

/// Wrapper function
/// Allocate `frame_count` contiguous page frames.
pub fn alloc_frames(frame_count: usize) -> PhysFrameRange {
    FREE_FRAMES.fetch_sub(frame_count, Ordering::SeqCst);
    frames_impl::alloc(frame_count)
}

/// Wrapper function
/// Free a contiguous range of page `frames`.
pub fn free_frames(frames: PhysFrameRange) {
    FREE_FRAMES.fetch_add((frames.end - frames.start) as usize, Ordering::SeqCst);
    frames_impl::free(frames);
}

/// Wrapper function
pub fn frame_allocator_locked() -> bool {
    frames_impl::allocator_locked()
}

/// Wrapper function
pub fn get_total_free_frames() -> usize {
    frames_impl::get_total_free_frames()
}


/// Helper function to convert a u64 address to a PhysFrame.
/// The given address is aligned up to the page size (4 KiB).
pub(super) fn frame_from_u64(addr: u64) -> Result<PhysFrame<Size4KiB>, x86_64::structures::paging::page::AddressNotAligned> {
    let pa = PhysAddr::new(addr).align_up(PAGE_SIZE as u64);
    PhysFrame::from_start_address(pa)
}