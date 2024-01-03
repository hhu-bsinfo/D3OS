use crate::kernel::Service;
use acpi::PhysicalMapping;
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ptr::NonNull;
use linked_list_allocator::LockedHeap;

pub struct KernelAllocator {
    heap: LockedHeap,
}

#[derive(Default, Clone)]
pub struct AcpiHandler;

#[derive(Clone)]
pub struct AcpiAllocator<'a> {
    allocator: &'a dyn Allocator,
}

unsafe impl<'a> Allocator for AcpiAllocator<'a> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.allocator.allocate(layout)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.allocator.deallocate(ptr, layout)
    }
}

impl Service for KernelAllocator {}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        return self
            .heap
            .lock()
            .allocate_first_fit(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr());
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.heap
            .lock()
            .deallocate(NonNull::new_unchecked(ptr), layout);
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
            self.heap.lock().deallocate(ptr, layout);
        }
    }
}

impl acpi::AcpiHandler for AcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        PhysicalMapping::new(
            physical_address,
            NonNull::new(physical_address as *mut T).unwrap(),
            size,
            size,
            AcpiHandler,
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

impl<'a> AcpiAllocator<'a> {
    pub fn new(allocator: &'a dyn Allocator) -> Self {
        Self { allocator }
    }
}

impl KernelAllocator {
    pub const fn new() -> Self {
        Self {
            heap: LockedHeap::empty(),
        }
    }

    pub unsafe fn init(&self, heap_start_address: usize, heap_end_address: usize) {
        self.heap.lock().init(
            heap_start_address as *mut u8,
            heap_end_address - heap_start_address,
        );
    }

    pub fn is_initialized(&self) -> bool {
        return self.heap.lock().size() > 0;
    }
}
