/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: stack                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Memory for a stack (user or kernel). The stack will be accessed within  ║
   ║ the kernel through a Vec and thus a Allocator is required.              ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - alloc_kernel_stack      alloc frames for a kernel stack             ║
   ║   - alloc_user_stack        alloc page range for a user stack           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, HHU, 25.05.2025            ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use crate::consts::KERNEL_STACK_PAGES;
use crate::memory::frames::phys_limit;
use crate::memory::{PAGE_SIZE, frames};
use alloc::vec::Vec;
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
use log::info;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::{PhysAddr, VirtAddr};

/// Allocate frames for a kernel stack for a thread with the given `pid` and `tid`.
pub fn alloc_kernel_stack(pid: usize, tid: usize) -> Vec<u64, StackAllocator> {
    let frames: PhysFrameRange = frames::alloc(KERNEL_STACK_PAGES);
    let mut kernel_stack = unsafe {
        Vec::from_raw_parts_in(
            frames.start.start_address().as_u64() as *mut u64,
            KERNEL_STACK_PAGES * PAGE_SIZE / 8,
            KERNEL_STACK_PAGES * PAGE_SIZE / 8,
            StackAllocator::new(
                pid,
                tid,
                true,
                frames.start.start_address().as_u64() as usize,
                frames.end.start_address().as_u64() as usize,
            ),
        )
    };
    kernel_stack.clear(); // Clear the stack to avoid garbage values
    kernel_stack
}

/// Allocate page range for a user stack for a thread with the given `pid` and `tid`. \
/// The first page begins at `start_addr` and the size of the stack is `size_in_bytes`.
pub fn alloc_user_stack(pid: usize, tid: usize, start_addr: usize, size_in_bytes: usize) -> Vec<u64, StackAllocator> {
    // Create Vec for user stack (backed by stack allocator)
    let user_stack = unsafe {
        Vec::from_raw_parts_in(
            start_addr as *mut u64,
            size_in_bytes / 8,
            size_in_bytes / 8,
            StackAllocator::new(pid, tid, false, start_addr, start_addr + size_in_bytes),
        )
    };
    user_stack
}

pub struct StackAllocator {
    pid: usize, // process id the stack belongs to
    tid: usize, // thread id the stack belongs to
    kernel: bool,
    start_addr: AtomicUsize, // start address of the first page used for the stack
    end_addr: AtomicUsize,   // end address of the last page used for the stack
}

impl StackAllocator {
    pub fn new(pid: usize, tid: usize, kernel: bool, start_addr: usize, end_addr: usize) -> Self {
        // Ensure that start_addr and end_addr are page-aligned
        StackAllocator {
            pid,
            tid,
            kernel,
            start_addr: AtomicUsize::new(start_addr),
            end_addr: AtomicUsize::new(end_addr),
        }
    }

    pub fn get_tid(&self) -> usize {
        self.tid
    }

    pub fn get_pid(&self) -> usize {
        self.pid
    }

    pub fn get_start_page(&self) -> Page {
        let start_addr = self.start_addr.load(Ordering::SeqCst);
        Page::from_start_address(VirtAddr::new(start_addr as u64)).unwrap()
    }

    pub fn get_end_page(&self) -> Page {
        let end_addr = self.end_addr.load(Ordering::SeqCst);
        Page::from_start_address(VirtAddr::new(end_addr as u64)).unwrap()
    }

    pub fn get_num_pages(&self) -> u64 {
        let start_addr = self.start_addr.load(Ordering::SeqCst);
        let end_addr = self.end_addr.load(Ordering::SeqCst);
        ((end_addr - start_addr) as u64) / PAGE_SIZE as u64
    }
}

unsafe impl Allocator for StackAllocator {

    /// Allocate is never called. It is required for the Vec. 
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        return Err(AllocError);


        
/*
        info!(
            "*** StackAllocator::allocate() called for stack of pid = {}, tid = {}, kernel = {:?}",
            self.pid, self.tid, self.kernel
        );

        if PAGE_SIZE % layout.align() != 0 {
            return Err(AllocError);
        }
        let frame_count = if layout.size() % PAGE_SIZE == 0 {
            layout.size() / PAGE_SIZE
        } else {
            (layout.size() / PAGE_SIZE) + 1
        };
        let frames: PhysFrameRange = frames::alloc(frame_count);

        self.start_addr.store(
            frames.start.start_address().as_u64() as usize,
            Ordering::SeqCst,
        );
        self.end_addr.store(
            frames.end.start_address().as_u64() as usize,
            Ordering::SeqCst,
        );

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(frames.start.start_address().as_u64() as *mut u8).unwrap(),
            (frames.end - frames.start) as usize * PAGE_SIZE,
        ))
        */
    }

    /// Deallocate is called when a thread terminates.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        info!(
            "deallocate() called for stack of pid = {}, tid = {}, kernel = {:?}",
            self.pid, self.tid, self.kernel
        );
        // Ignore virtual addresses
        if (ptr.as_ptr() as usize) < phys_limit().start_address().as_u64() as usize {
            assert_eq!(PAGE_SIZE % layout.align(), 0);
            assert_eq!(layout.size() % PAGE_SIZE, 0);

            let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64)).unwrap();
            unsafe {
                frames::free(PhysFrameRange {
                    start,
                    end: start + (layout.size() / PAGE_SIZE) as u64,
                });
            }
        }
    }
}
