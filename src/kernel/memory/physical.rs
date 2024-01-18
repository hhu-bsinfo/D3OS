use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::ptr;
use log::{debug, info};
use spin::{Mutex, Once};
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::PhysFrame;
use crate::kernel::memory::{KERNEL_PHYS_LIMIT, MemorySpace, PAGE_SIZE};

static KERNEL_PAGE_FRAME_ALLOCATOR: Mutex<PageFrameListAllocator> = Mutex::new(PageFrameListAllocator::new());
static USER_PAGE_FRAME_ALLOCATOR: Mutex<PageFrameListAllocator> = Mutex::new(PageFrameListAllocator::new());
static PHYS_LIMIT: Once<PhysFrame> = Once::new();

/// Initialize page frame allocation with available memory regions, obtained during the boot process.
pub unsafe fn init(mut regions: Vec<PhysFrameRange>, kernel_heap_end: PhysFrame) {
    regions.sort_by(|range1, range2| range1.start.cmp(&range2.start));
    PHYS_LIMIT.call_once(|| regions.iter().max_by(|region1, region2| region1.end.cmp(&region2.end)).unwrap().end);
    info!("Available physical memory: [{} MiB]", PHYS_LIMIT.get().unwrap().start_address().as_u64() / 1024 / 1024);

    // Calculate memory required for page tables to map the whole physical memory
    let page_table_memory = calc_page_table_memory(4);

    // Set kernel limit to heap end
    let mut kernel_phys_limit = kernel_heap_end;

    // Calculate physical kernel limit
    let mut available_kernel_memory = 0;
    let mut region_iter = regions.iter();
    while available_kernel_memory < page_table_memory {
        let required_memory = page_table_memory - available_kernel_memory;
        if required_memory <= 0 {
            break;
        }

        let region = region_iter.next().expect("Not enough physical memory for required page tables available!");
        if region.count() * PAGE_SIZE >= required_memory {
            // The region is larger than the required memory, so we only use a part of it for the kernel
            available_kernel_memory = available_kernel_memory + required_memory;
            kernel_phys_limit = region.start + (required_memory / PAGE_SIZE) as u64;
        } else {
            // Use the full region for the kernel
            available_kernel_memory = available_kernel_memory + region.count() * PAGE_SIZE;
            kernel_phys_limit = region.end;
        };
    }

    // Physical kernel memory must include kernel heap
    if kernel_phys_limit < kernel_heap_end {
        kernel_phys_limit = kernel_heap_end;
    }

    info!("Physical kernel memory: [{} MiB]", kernel_phys_limit.start_address().as_u64() / 1024 / 1024);
    KERNEL_PHYS_LIMIT.call_once(|| kernel_phys_limit);

    for mut region in regions {
        // Check if the given region transcends over the physical kernel limit
        if region.start < kernel_phys_limit && region.end >= kernel_phys_limit {
            // Insert region partially up to the physical kernel limit
            let kernel_region = PhysFrameRange { start: region.start, end: kernel_phys_limit };
            free(kernel_region);

            // Calculate remaining region
            region = PhysFrameRange { start: kernel_phys_limit, end: region.end };
        }

        free(region);
    }


    debug!("Kernel page frame allocator:\n{:?}", KERNEL_PAGE_FRAME_ALLOCATOR.lock());
    debug!("User page frame allocator:\n{:?}", USER_PAGE_FRAME_ALLOCATOR.lock());
}

/// Allocate `frame_count` contiguous page frames in either kernel or user space, depending on `space`.
pub fn alloc(frame_count: usize, space: MemorySpace) -> PhysFrameRange {
    unsafe {
        return match space {
            MemorySpace::Kernel => KERNEL_PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count),
            MemorySpace::User => USER_PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count)
        }
    }
}

/// Free `frame_count` contiguous page frames starting at `addr`.
/// Unsafe because invalid parameters may break the list allocator.
pub unsafe fn free(frames: PhysFrameRange) {
    if frames.start < kernel_phys_limit() {
        KERNEL_PAGE_FRAME_ALLOCATOR.lock().free_block(frames);
    } else {
        USER_PAGE_FRAME_ALLOCATOR.lock().free_block(frames);
    }
}

pub fn phys_limit() -> PhysFrame {
    return *PHYS_LIMIT.get().expect("PageFrameAllocator: 'PHYS_LIMIT' accessed before initialization!");
}

pub fn kernel_phys_limit() -> PhysFrame {
    return *KERNEL_PHYS_LIMIT.get().expect("PageFrameAllocator: 'KERNEL_PHYS_LIMIT' accessed before initialization!");
}

fn calc_page_table_memory(levels: usize) -> usize {
    let available_memory: usize = phys_limit().start_address().align_up(0x200000u64).as_u64() as usize;

    let mut page_table_sizes = Vec::<usize>::with_capacity(levels);
    for level in 0..levels {
        page_table_sizes.push(0);
        if level == 0 {
            page_table_sizes[level] = available_memory / PAGE_SIZE / 512
        } else {
            page_table_sizes[level] = page_table_sizes[level - 1] / 512;
            if page_table_sizes[level] == 0 {
                page_table_sizes[level] = 1;
            }
        }
    }

    let needed_memory = page_table_sizes.iter().sum::<usize>() * PAGE_SIZE;
    debug!("Page table sizes required to map physical memory: {:?}", page_table_sizes);
    debug!("Required page table memory: [{} KiB]", needed_memory / 1024);

    return needed_memory;
}

/// Entry in the free list.
/// Represents a block of available physical memory.
struct PageFrameNode {
    frame_count: usize,
    next: Option<&'static mut PageFrameNode>
}

impl PageFrameNode {
    const fn new(frame_count: usize) -> Self {
        Self { frame_count, next: None }
    }

    fn range(&self) -> PhysFrameRange {
        let start = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(self) as u64)).unwrap();
        let end = start + self.frame_count as u64;

        return PhysFrameRange { start, end };
    }

    fn start(&self) -> PhysFrame {
        return PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(self) as u64)).unwrap();
    }

    fn end(&self) -> PhysFrame {
        return self.start() + self.frame_count as u64;
    }
}

/// Manages block of available physical memory as a linked list
/// Since each page frame is exactly 4 KiB large, allocations are always a multiple of 4096.
struct PageFrameListAllocator {
    head: PageFrameNode
}

impl Debug for PageFrameListAllocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut available: usize = 0;

        let mut current = &self.head;
        while let Some(block) = &current.next {
            write!(f, "{:?}\n", block.range())?;
            available = available + block.frame_count;

            current = current.next.as_ref().unwrap();
        }

        write!(f, "Available memory: [{} KiB]", available * PAGE_SIZE / 1024)
    }
}

impl PageFrameListAllocator {
    pub const fn new() -> Self {
        Self { head: PageFrameNode::new(0) }
    }

    /// Insert a new block, sorted ascending by its memory address.
    unsafe fn insert(&mut self, frames: PhysFrameRange) {
        let mut new_block = PageFrameNode::new(frames.count());
        let new_block_ptr = frames.start.start_address().as_u64() as *mut PageFrameNode;

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
            if block.start().start_address() > frames.start.start_address() {
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
    fn find_free_block(&mut self, frame_count: usize) -> Option<&'static mut PageFrameNode> {
        let mut current = &mut self.head;
        while let Some(ref mut block) = current.next {
            if block.frame_count >= frame_count {
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
    unsafe fn alloc_block(&mut self, frame_count: usize) -> PhysFrameRange {
        match self.find_free_block(frame_count) {
            Some(block) => {
                let remaining = PhysFrameRange { start: block.start() + frame_count as u64, end: block.end() };
                if remaining.count() > 0 {
                    self.insert(remaining);
                }
                
                return PhysFrameRange { start: block.start(), end: remaining.start };
            },
            None => panic!("PageFrameAllocator: Out of memory!")
        }
    }

    /// Free a block of memory, consisting of at least one page frame.
    /// The block is inserted ascending by address and fused with its neighbours, if possible.
    unsafe fn free_block(&mut self, frames: PhysFrameRange) {
        let mut current = &mut self.head;
        let new_block_ptr: *mut PageFrameNode;

        // Run through list and check if fusion is possible
        while let Some(ref mut block) = current.next {
            if frames.end == block.start() {
                // The freed memory block extends 'block' from the bottom
                let mut new_block = PageFrameNode::new(block.frame_count + frames.count());
                new_block_ptr = frames.start.start_address().as_u64() as *mut PageFrameNode;
                new_block.next = block.next.take();
                new_block_ptr.write(new_block);

                return;
            } else if block.end() == frames.start {
                // The freed memory block extends 'block' from the top
                let new_block = PageFrameNode::new(block.frame_count + frames.count());
                new_block_ptr = block.start().start_address().as_u64() as *mut PageFrameNode;
                new_block_ptr.write(new_block);

                return;
            } else if block.end() > frames.start {
                // The freed memory block does not extend any existing block and needs a new entry in the list
                break;
            }

            current = current.next.as_mut().unwrap();
        }

        self.insert(frames);
    }
}