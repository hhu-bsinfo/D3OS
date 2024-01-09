use alloc::vec::Vec;
use core::ptr;
use log::debug;
use spin::{Mutex, Once};
use x86_64::PhysAddr;
use crate::kernel::memory::{KERNEL_PHYS_SIZE, PAGE_SIZE};

static KERNEL_PAGE_FRAME_ALLOCATOR: Mutex<PageFrameListAllocator> = Mutex::new(PageFrameListAllocator::new());
static USER_PAGE_FRAME_ALLOCATOR: Mutex<PageFrameListAllocator> = Mutex::new(PageFrameListAllocator::new());
static MAX_PHYSICAL_ADDRESS: Once<PhysAddr> = Once::new();

pub enum MemorySpace {
    Kernel,
    User
}

pub struct MemoryRegion {
    start: PhysAddr,
    end: PhysAddr
}

impl MemoryRegion {
    pub const fn new(start: PhysAddr, end: PhysAddr) -> Self {
        Self { start, end }
    }

    pub fn from_size(start: PhysAddr, size: usize) -> Self {
        Self { start, end: start + (size - 1) }
    }

    pub fn start(&self) -> PhysAddr {
        return self.start;
    }

    pub fn end(&self) -> PhysAddr {
        return self.end;
    }

    pub fn size(&self) -> usize {
        return (self.end - self.start + 1) as usize;
    }
}

/// Entry in the free list.
/// Represents a block of available physical memory.
struct PageFrameNode {
    size: usize,
    next: Option<&'static mut PageFrameNode>
}

impl PageFrameNode {
    const fn new(size: usize) -> Self {
        Self { size, next: None }
    }

    fn start(&self) -> PhysAddr {
        return PhysAddr::new(ptr::from_ref(self) as u64);
    }

    fn end(&self) -> PhysAddr {
        return self.start() + self.size - 1u64;
    }
}

/// Manages block of available physical memory as a linked list
/// Since each page frame is exactly 4 KiB large, allocations must a multiple for 4096.
struct PageFrameListAllocator {
    head: PageFrameNode
}

impl PageFrameListAllocator {
    pub const fn new() -> Self {
        Self { head: PageFrameNode::new(0) }
    }

    /// Insert a new block, sorted ascending by its memory address.
    unsafe fn insert(&mut self, addr: PhysAddr, size: usize) {
        assert_eq!(addr.as_u64() % PAGE_SIZE as u64, 0);
        assert_eq!(size % PAGE_SIZE, 0);

        let mut new_block = PageFrameNode::new(size);
        let new_block_ptr = addr.as_u64() as *mut PageFrameNode;

        // Check if list is empty
        if self.head.next.is_none() {
            new_block.next = self.head.next.take();
            new_block_ptr.write(new_block);
            self.head.next = Some(&mut *new_block_ptr);

            return;
        }

        // List is not empty -> Search for correct position and insert new block
        let mut current = &mut self.head;
        while let Some(ref mut block) = current.next {
            if block.start() > addr {
                new_block.next = current.next.take();
                new_block_ptr.write(new_block);
                current.next = Some(&mut *new_block_ptr);

                return;
            }

            current = current.next.as_mut().unwrap()
        }

        // Insert new block at the list's end
        new_block.next = None;
        new_block_ptr.write(new_block);
        current.next = Some(&mut *new_block_ptr);
    }

    /// Search a free memory block.
    fn find_free_block(&mut self, size: usize) -> Option<&'static mut PageFrameNode> {
        let mut current = &mut self.head;
        while let Some(ref mut block) = current.next {
            if block.size >= size {
                let next = block.next.take();
                let ret = Some(current.next.take().unwrap());
                current.next = next;

                return ret;
            } else {
                // Block to small -> Continue with next block
                current = current.next.as_mut().unwrap();
            }
        }

        return None;
    }

    /// Allocate `frame_count` page frames.
    unsafe fn alloc_block(&mut self, frame_count: usize) -> PhysAddr {
        let size = frame_count * PAGE_SIZE;

        match self.find_free_block(size) {
            Some(block) => {
                let alloc_end = block.start() + (size - 1);
                let remaining_block_size = block.end() - alloc_end;
                if remaining_block_size > 0 {
                    self.insert(alloc_end + 1u64, remaining_block_size as usize);
                }
                
                return block.start();
            },
            None => panic!("PageFrameAllocator: Out of memory!")
        }
    }

    /// Free a block of memory, consisting of at least one page frame.
    /// The block is inserted ascending by address and fused with its neighbours, if possible.
    unsafe fn free_block(&mut self, addr: PhysAddr, frame_count: usize) {
        assert_eq!(addr.as_u64() % PAGE_SIZE as u64, 0);

        let free_start = addr;
        let free_end = addr - 1u64 + frame_count * PAGE_SIZE;

        let mut current = &mut self.head;
        let new_block_ptr: *mut PageFrameNode;

        // Run through list and check if fusion is possible
        while let Some(ref mut block) = current.next {
            if free_end + 1u64 == block.start() {
                // The freed memory block extends 'block' from the bottom
                let mut new_block = PageFrameNode::new(block.size + frame_count * PAGE_SIZE);
                new_block_ptr = free_start.as_u64() as *mut PageFrameNode;
                new_block.next = block.next.take();
                new_block_ptr.write(new_block);

                return;
            } else if block.end() + 1u64 == free_start {
                // The freed memory block extends 'block' from the top
                let new_block = PageFrameNode::new(block.size + frame_count * PAGE_SIZE);
                new_block_ptr = block.start().as_u64() as *mut PageFrameNode;
                new_block_ptr.write(new_block);

                return;
            } else if block.end() + 1u64 < free_start {
                // The freed memory block does not extend any existing block and needs a new entry in the list
                break;
            }

            current = current.next.as_mut().unwrap();
        }

        self.insert(addr, frame_count * PAGE_SIZE);
    }
}

pub fn max_physical_address() -> PhysAddr {
    return *MAX_PHYSICAL_ADDRESS.get().expect("PageFrameAllocator: MAX_PHYSICAL_ADDRESS accessed before initialization!");
}

/// Initialize page frame allocation with available memory regions, obtained during the boot process.
pub unsafe fn init(regions: Vec<MemoryRegion>) {
    let mut max_phys_addr: PhysAddr = PhysAddr::zero();

    for mut region in regions {
        // Skip invalid entries
        if region.start >= region.end {
            continue;
        }

        // Check if the given region transcends over the physical kernel limit
        if region.start < KERNEL_PHYS_SIZE && region.end >= KERNEL_PHYS_SIZE {
            // Insert region partially up to the physical kernel limit
            let kernel_region = MemoryRegion::new(region.start, KERNEL_PHYS_SIZE - 1u64);
            insert_memory_region(kernel_region);

            // Calculate remaining region
            region = MemoryRegion::new(KERNEL_PHYS_SIZE, region.end);
        }

        if region.end > max_phys_addr {
            max_phys_addr = region.end;
        }

        insert_memory_region(region);
    }

    MAX_PHYSICAL_ADDRESS.call_once(|| max_phys_addr);
}

/// Allocate `frame_count` contiguous page frames in either kernel or user space, depending on `space`.
pub fn alloc(frame_count: usize, space: MemorySpace) -> PhysAddr {
    unsafe {
        return match space {
            MemorySpace::Kernel => KERNEL_PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count),
            MemorySpace::User => USER_PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count)
        }
    }
}

/// Free `frame_count` contiguous page frames starting at `addr`.
/// Unsafe because invalid parameters may break the list allocator.
pub unsafe fn free(addr: PhysAddr, frame_count: usize) {
    if addr < KERNEL_PHYS_SIZE {
        KERNEL_PAGE_FRAME_ALLOCATOR.lock().free_block(addr, frame_count);
    } else {
        USER_PAGE_FRAME_ALLOCATOR.lock().free_block(addr, frame_count);
    }
}

/// Insert a memory region into the list. Start and end addresses of each region are aligned before insertion.
unsafe fn insert_memory_region(region: MemoryRegion) {
    let aligned_region = MemoryRegion::new(region.start.align_down(PAGE_SIZE as u64), region.end.align_up(PAGE_SIZE as u64) - 1u64);
    debug!("Inserting region (Start: 0x{:0>16x}, End: 0x{:0>16x})", aligned_region.start.as_u64(), aligned_region.end.as_u64());

    if aligned_region.start > (KERNEL_PHYS_SIZE - 1u64) {
        USER_PAGE_FRAME_ALLOCATOR.lock().insert(aligned_region.start, aligned_region.size());
    } else {
        KERNEL_PAGE_FRAME_ALLOCATOR.lock().insert(aligned_region.start, aligned_region.size());
    }
}