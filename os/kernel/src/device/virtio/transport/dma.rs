use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::Page;
use x86_64::structures::paging::page::PageRange;
use crate::memory::frames;

pub const PAGE_SIZE: usize = 0x1000;

#[derive(Debug)]
pub struct DmaBuffer {
    paddr: PhysAddr,
    vaddr: VirtAddr,
    size: usize,
    pages: usize,
}


// The Send trait indicates that it is safe to transfer ownership of a DmaTransport instance to another thread.
unsafe impl Send for DmaBuffer {}
// The Sync trait indicates that it is safe to reference a DmaTransport instance from multiple threads.
unsafe impl Sync for DmaBuffer {}

impl DmaBuffer {
    pub fn new(pages: usize) -> Self {
        let size = pages * PAGE_SIZE;

        let phys_buffer = frames::alloc(size);
        let phys_start_addr = phys_buffer.start.start_address();
        let page_range = PageRange {
            start: Page::from_start_address(VirtAddr::new(phys_start_addr.as_u64())).unwrap(),
            end: Page::from_start_address(VirtAddr::new(phys_buffer.end.start_address().as_u64())).unwrap()
        };
        Self {
            paddr: phys_start_addr,
            vaddr: page_range.start.start_address(),
            size,
            pages,
        }
    }

    pub fn paddr(&self) -> PhysAddr {
        self.paddr
    }

    pub fn vaddr(&self) -> VirtAddr {
        self.vaddr
    }

    pub fn pages(&self) -> usize {
        self.pages
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BufferDirection {
    /// The buffer can be accessed for reading and writing by the driver but is read-only for the device.
    DriverToDevice,
    /// The buffer can be accessed for reading and writing by the device but is read-only for the driver.
    DeviceToDriver,
    /// The buffer can be read from and written to by both the device and the driver.
    Both,
}