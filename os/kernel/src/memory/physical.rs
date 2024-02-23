use alloc::format;
use alloc::string::String;
use core::cell::Cell;
use core::fmt::{Debug, Formatter};
use core::ptr;
use spin::Mutex;
use spin::once::Once;
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::PhysFrame;
use crate::memory::PAGE_SIZE;

static PAGE_FRAME_ALLOCATOR: Mutex<PageFrameListAllocator> = Mutex::new(PageFrameListAllocator::new());
static PHYS_LIMIT: Once<Mutex<Cell<PhysFrame>>> = Once::new();

/// Insert an available memory region obtained during the boot process.
pub unsafe fn insert(mut region: PhysFrameRange) {
    PHYS_LIMIT.call_once(|| Mutex::new(Cell::new(PhysFrame::from_start_address(PhysAddr::zero()).unwrap())));

    // Make sure, the first page is not inserted to avoid null pointer panics
    if region.start.start_address() == PhysAddr::zero() {
        let first_page = PhysFrame::from_start_address(PhysAddr::new(PAGE_SIZE as u64)).unwrap();
        if region.end <= first_page { // Region only contains the first page -> Skip insertion
            return;
        }

        region.start = first_page; // Cut first page out of region and continue
    }

    let current_limit = PHYS_LIMIT.get().unwrap().lock();
    if region.end > current_limit.get() {
        current_limit.swap(&Cell::new(region.end));
    }

    free(region);
}

/// Allocate `frame_count` contiguous page frames.
pub fn alloc(frame_count: usize) -> PhysFrameRange {
    PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count)
}

/// Free `frame_count` contiguous page frames.
/// Unsafe because invalid parameters may break the list allocator.
pub unsafe fn free(frames: PhysFrameRange) {
    PAGE_FRAME_ALLOCATOR.lock().free_block(frames);
}

/// Permanently reserve a block of free memory.
pub unsafe fn reserve(frames: PhysFrameRange) {
    PAGE_FRAME_ALLOCATOR.lock().reserve_block(frames);
}

/// Get the highest physical address, managed by PAGE_FRAME_ALLOCATOR.
pub fn phys_limit() -> PhysFrame {
    return PHYS_LIMIT.get().unwrap().lock().get();
}

/// Get a dump of the current free list.
pub fn dump() -> String {
    format!("{:?}", PAGE_FRAME_ALLOCATOR.lock())
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

    fn start(&self) -> PhysFrame {
        return PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(self) as u64)).unwrap();
    }

    fn end(&self) -> PhysFrame {
        return self.start() + self.frame_count as u64;
    }
}

/// Manages blocks of available physical memory as a linked list
/// Since each page frame is exactly 4 KiB large, allocations are always a multiple of 4096.
struct PageFrameListAllocator {
    head: PageFrameNode
}

impl Debug for PageFrameListAllocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut available: usize = 0;

        let mut current = &self.head;
        while let Some(block) = &current.next {
            write!(f, "Block: [0x{:x} - 0x{:x}], Frame count: [{}]\n", block.start().start_address().as_u64(), block.end().start_address().as_u64(), block.end() - block.start())?;
            available = available + block.frame_count;

            current = current.next.as_ref().unwrap();
        }

        write!(f, "Available memory: [{} KiB]\n", available * PAGE_SIZE / 1024)?;
        write!(f, "Physical limit: [0x{:0>16x}]", PHYS_LIMIT.get().unwrap().lock().get().start_address().as_u64())
    }
}

impl PageFrameListAllocator {
    pub const fn new() -> Self {
        Self { head: PageFrameNode::new(0) }
    }

    /// Insert a new block, sorted ascending by its memory address.
    unsafe fn insert(&mut self, frames: PhysFrameRange) {
        let mut new_block = PageFrameNode::new((frames.end - frames.start) as usize);
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
    fn alloc_block(&mut self, frame_count: usize) -> PhysFrameRange {
        match self.find_free_block(frame_count) {
            Some(block) => {
                let remaining = PhysFrameRange { start: block.start() + frame_count as u64, end: block.end() };
                if (remaining.end - remaining.start) > 0 {
                    unsafe { self.insert(remaining); }
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
                let mut new_block = PageFrameNode::new(block.frame_count + (frames.end - frames.start) as usize);
                new_block_ptr = frames.start.start_address().as_u64() as *mut PageFrameNode;
                new_block.next = block.next.take();
                new_block_ptr.write(new_block);

                current.next = Some(&mut *new_block_ptr);
                return;
            } else if block.end() == frames.start {
                // The freed memory block extends 'block' from the top
                block.frame_count += (frames.end - frames.start) as usize;

                // The extended 'block' may now extend its successor from the bottom
                let end = block.end();
                if let Some(ref mut next) = block.next {
                    if end == next.start() {
                        block.frame_count += next.frame_count;
                        block.next = next.next.take();
                    }
                }

                return;
            } else if block.end() > frames.start {
                // The freed memory block does not extend any existing block and needs a new entry in the list
                break;
            }

            current = current.next.as_mut().unwrap();
        }

        self.insert(frames);
    }

    /// Permanently reserve a block of free memory.
    unsafe fn reserve_block(&mut self, reserved: PhysFrameRange) {
        let mut current = &mut self.head;

        // Run through list and search for free blocks, containing the reserved block
        while let Some(ref mut block) = current.next {

            if block.start() > reserved.end { // Block lies completely above reserved region and since the list is sorted, we can abort
                break;
            } else if block.start() < reserved.start && block.end() >= reserved.start { // Block starts below the reserved region
                if block.end() <= reserved.end { // Block starts below and ends inside the reserved region
                    let overlapping = block.end() - reserved.start;
                    block.frame_count -= overlapping as usize;
                } else { // Block starts below and ends above the reserved region
                    let below_size = reserved.start - block.start();
                    let above_size = block.end() - reserved.end;
                    block.frame_count = below_size as usize;

                    let mut above_block = PageFrameNode::new(above_size as usize);
                    let above_block_ptr = reserved.end.start_address().as_u64() as *mut PageFrameNode;
                    above_block.next = block.next.take();
                    block.next = Some(&mut *above_block_ptr);
                    above_block_ptr.write(above_block);
                }
            } else if block.start() <= reserved.end && block.end() >= reserved.start { // Block starts within the reserved region
                if block.end() <= reserved.end { // Block start within and ends within the reserved region
                    current.next = block.next.take();
                } else { // Block starts within and ends above the reserved region
                    let overlapping = reserved.end - block.start();
                    let mut new_block = PageFrameNode::new(block.frame_count - overlapping as usize);
                    let new_block_ptr = (block.start() + overlapping).start_address().as_u64() as *mut PageFrameNode;

                    new_block.next = block.next.take();
                    new_block_ptr.write(new_block);
                    current.next = Some(&mut *new_block_ptr);
                }
            }

            current = current.next.as_mut().unwrap();
        }
    }
}