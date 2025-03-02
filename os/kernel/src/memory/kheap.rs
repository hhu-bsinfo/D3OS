use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;
use linked_list_allocator::LockedHeap;
use x86_64::structures::paging::frame::PhysFrameRange;
use crate::memory::PAGE_SIZE;

pub struct KernelAllocator {
    heap: LockedHeap,
}

impl KernelAllocator {
    pub const fn new() -> Self {
        Self { heap: LockedHeap::empty() }
    }

    pub unsafe fn init(&self, frames: &PhysFrameRange) {
        let mut heap = self.heap.lock();
        unsafe { heap.init(frames.start.start_address().as_u64() as *mut u8, (frames.end - frames.start) as usize * PAGE_SIZE); }
    }

    pub fn is_initialized(&self) -> bool {
        self.heap.lock().size() > 0
    }

    pub fn is_locked(&self) -> bool {
        self.heap.is_locked()
    }
}

unsafe impl Allocator for KernelAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() == 0 {
            return Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0));
        }

        match self.heap.lock().allocate_first_fit(layout) {
            Ok(ptr) => Ok(NonNull::slice_from_raw_parts(ptr, layout.size())),
            Err(()) => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != 0 {
            let mut heap = self.heap.lock();
            unsafe { heap.deallocate(ptr, layout); }
        }
    }
}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.heap.lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock();
        unsafe { heap.deallocate(NonNull::new_unchecked(ptr), layout); }
    }
}

