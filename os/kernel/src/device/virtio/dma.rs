/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: dma                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ DMA buffer allocation for Virtio devices.                               ║
   ║ Provides memory regions that are physically contiguous,                 ║
   ║ identity-mapped, and uncached for safe device access.                   ║
   ║                                                                         ║
   ║ Key Functions:                                                          ║
   ║   - new             allocate uncached DMA memory for device access      ║
   ║   - paddr           return physical address of the DMA buffer           ║
   ║   - vaddr           return virtual address with byte offset             ║
   ║   - raw_slice       get the full memory slice as NonNull<[u8]>          ║
   ║                                                                         ║
   ║ Spec: Virtio 1.3, §2.4.2 – Driver DMA requirements                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Nikita E., Univ. Duesseldorf, 2025                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::ptr::NonNull;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{Page, PageTableFlags, page::PageRange};

use crate::memory;
use crate::memory::PAGE_SIZE;
use crate::process_manager;

/// A DMA buffer allocated in physical memory and mapped into virtual address space.
#[derive(Debug)]
pub struct Dma {
    paddr: PhysAddr,
    vaddr: NonNull<u8>,
    size: usize,
    pages: usize,
}

impl Dma {
    /// Allocates a new DMA buffer consisting of the given number of 4KiB pages.
    ///
    /// - Allocates physical frames
    /// - Disables caching (important for DMA correctness)
    /// - Uses identity mapping to get virtual address
    ///
    /// # Panics
    /// Panics if `pages == 0` or allocation fails (i.e., `paddr == 0`).
    ///
    /// Spec reference: Virtio 1.3 §2.4.2: DMA memory must be mapped and accessible for the device.
    pub fn new(pages: usize) -> Self {
        assert!(pages > 0, "DMA must allocate at least one page");

        // Allocate physical memory frames.
        let phys_frames = memory::alloc_frames(pages);
        let paddr = phys_frames.start.start_address();

        // Define the virtual page range corresponding to the allocated frames.
        let pages_range = PageRange {
            start: Page::from_start_address(VirtAddr::new(phys_frames.start.start_address().as_u64())).unwrap(),
            end: Page::from_start_address(VirtAddr::new(phys_frames.end.start_address().as_u64())).unwrap(),
        };

        // Mark the pages as uncached for DMA consistency.
        let kernel_process = process_manager().read().kernel_process().unwrap();
        kernel_process.virtual_address_space.set_flags(
            pages_range,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE,
        );

        // In D3OS, the kernel is identity-mapped: physical = virtual address.
        let vaddr = NonNull::new(paddr.as_u64() as *mut u8).expect("Invalid virtual address");

        if paddr.as_u64() == 0 {
            panic!("Failed to allocate DMA memory");
        }

        Self {
            paddr,
            vaddr,
            size: pages * PAGE_SIZE,
            pages,
        }
    }

    /// Returns the starting physical address of the DMA buffer.
    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }

    /// Returns the virtual address with a byte offset.
    ///
    /// # Panics
    /// Panics if the offset exceeds the size of the allocated buffer.
    pub fn vaddr(&self, offset: usize) -> NonNull<u8> {
        assert!(offset < self.size, "DMA offset out of bounds");
        unsafe {
            NonNull::new_unchecked(self.vaddr.as_ptr().add(offset))
        }
    }

    /// Returns the number of allocated pages.
    pub fn pages(&self) -> usize {
        self.pages
    }

    /// Returns the total size of the DMA buffer in bytes.
    pub fn size(&self) -> usize {
        self.size
    }

    /// Returns the virtual address as a `usize` from a raw buffer.
    ///
    /// # Safety
    /// Caller must ensure the buffer was allocated properly and is valid.
    pub unsafe fn get_pointer_to_vaddr(buffer: NonNull<[u8]>) -> usize {
        buffer.as_ptr() as *mut u8 as usize
    }

    /// Returns a raw `NonNull<[u8]>` slice covering the entire DMA region.
    pub fn raw_slice(&self) -> NonNull<[u8]> {
        let raw_slice = core::ptr::slice_from_raw_parts_mut(self.vaddr(0).as_ptr(), self.size);
        unsafe { NonNull::new_unchecked(raw_slice) }
    }
}

unsafe impl Sync for Dma {}
unsafe impl Send for Dma {}
