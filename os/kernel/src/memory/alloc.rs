use acpi::PhysicalMapping;
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;
use linked_list_allocator::LockedHeap;
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::PhysFrame;
use crate::memory::{PAGE_SIZE, physical};
use crate::memory::physical::phys_limit;

pub struct KernelAllocator {
    heap: LockedHeap,
}

pub struct StackAllocator {}

#[derive(Default, Clone)]
pub struct AcpiHandler;

impl KernelAllocator {
    pub const fn new() -> Self {
        Self { heap: LockedHeap::empty() }
    }

    pub unsafe fn init(&self, frames: &PhysFrameRange) {
        let mut heap = self.heap.lock();
        unsafe { heap.init(frames.start.start_address().as_u64() as *mut u8, (frames.end - frames.start) as usize * PAGE_SIZE); }
    }

    pub fn is_initialized(&self) -> bool {
        return self.heap.lock().size() > 0;
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
        return self.heap.lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr());
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut heap = self.heap.lock();
        unsafe { heap.deallocate(NonNull::new_unchecked(ptr), layout); }
    }
}

impl StackAllocator {
    pub const fn new() -> Self {
        Self {}
    }
}

unsafe impl Allocator for StackAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if PAGE_SIZE % layout.align() != 0 {
            return Err(AllocError);
        }

        let frame_count = if layout.size() % PAGE_SIZE == 0 { layout.size() / PAGE_SIZE } else { (layout.size() / PAGE_SIZE) + 1 };
        let frames = physical::alloc(frame_count);

        return Ok(NonNull::slice_from_raw_parts(NonNull::new(frames.start.start_address().as_u64() as *mut u8).unwrap(), (frames.end - frames.start) as usize * PAGE_SIZE))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Ignore virtual addresses
        if (ptr.as_ptr() as usize) < phys_limit().start_address().as_u64() as usize {
            assert_eq!(PAGE_SIZE % layout.align(), 0);
            assert_eq!(layout.size() % PAGE_SIZE, 0);

            let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64)).unwrap();
            unsafe { physical::free(PhysFrameRange { start, end: start + (layout.size() / PAGE_SIZE) as u64 }); }
        }
    }
}

impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(&self, physical_address: usize, size: usize) -> PhysicalMapping<Self, T> {
        unsafe { PhysicalMapping::new(physical_address, NonNull::new(physical_address as *mut T).unwrap(), size, size, AcpiHandler) }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}
