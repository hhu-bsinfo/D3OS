/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: stack                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Stack allocator for stacks and alloc functions.                         ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - alloc_kernel_stack      alloc frames for a kernel stack             ║
   ║   - alloc_user_stack        alloc page range for a user stack           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, HHU, 28.06.2025            ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
use log::info;
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::PageTableFlags;

use crate::consts::KERNEL_STACK_PAGES;
use crate::memory::vma::VmaType;
use crate::memory::PAGE_SIZE;
use crate::process::process::Process;

/// Allocate memory for a kernel stack for a thread with the given `pid` and `tid`.
/// A VMA is created in the address space of `process`.
pub fn alloc_kernel_stack(process: &Arc<Process>, pid: usize, tid: usize, tag_str: &str) -> Vec<u64, StackAllocator> {

    // Allocate physical frames for the kernel stack
    let start_page = process.virtual_address_space.kernel_alloc_map_identity(
        KERNEL_STACK_PAGES as u64,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        VmaType::KernelStack,
        tag_str,
    );

    // Create a Vec for the allocated kernel stack 
    let mut kernel_stack = unsafe {
        Vec::from_raw_parts_in(
            start_page.start.start_address().as_u64() as *mut u64,
            KERNEL_STACK_PAGES * PAGE_SIZE / 8,
            KERNEL_STACK_PAGES * PAGE_SIZE / 8,
            StackAllocator::new(
                pid,
                tid,
                start_page.start.start_address().as_u64() as usize,
                start_page.end.start_address().as_u64() as usize,
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
    unsafe {
        Vec::from_raw_parts_in(
            start_addr as *mut u64,
            size_in_bytes / 8,
            size_in_bytes / 8,
            StackAllocator::new(pid, tid, start_addr, start_addr + size_in_bytes),
        )
    }
}

pub struct StackAllocator {
    pid: usize, // process id the stack belongs to
    tid: usize, // thread id the stack belongs to
    start_addr: AtomicUsize, // start address of the first page used for the stack
    end_addr: AtomicUsize,   // end address of the last page used for the stack
}

impl StackAllocator {
    pub fn new(pid: usize, tid: usize, start_addr: usize, end_addr: usize) -> Self {
        // Ensure that start_addr and end_addr are page-aligned
        StackAllocator {
            pid,
            tid,
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
    /// Allocate should never be called. Memory is explictly allocated, see above functions.
    /// It is required for working with a Vec.
    fn allocate(&self, _layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        //   info!("Allocating stack memory for pid: {}, tid: {}", self.pid, self.tid);
        Err(AllocError)
    }

    /// Deallocate is called when a thread terminates.
    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // memory is automaticallyfreed when page tables are dropped */
        info!("Deallocating stack memory for pid: {}, tid: {}", self.pid, self.tid);
    }
}
