/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: physical                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Page frame allocator.                                                   ║
   ║   - alloc              allooc a range of frames                         ║
   ║   - allocator_locked   check if allocator is locked                     ║
   ║   - dump               get a dump of the current free list              ║
   ║   - free               free a range of frames                           ║
   ║   - insert             insert free frame region detected during boot    ║
   ║   - phys_limit         get the highest phys. addr. managed by the alloc.║
   ║   - reserve            permanently reserve a range of frames            ║
   ║   - frame_from_u64     convert a u64 address to a PhysFrame             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 24.5.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::memory::PAGE_SIZE;
use alloc::format;
use alloc::string::String;
use core::cell::Cell;
use core::fmt::{Debug, Formatter};
use core::ptr;
use log::info;
use spin::Mutex;
use spin::once::Once;
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::{PhysFrame, Size4KiB};

static PAGE_FRAME_ALLOCATOR: Mutex<PageFrameListAllocator> =
    Mutex::new(PageFrameListAllocator::new());
static PHYS_LIMIT: Once<Mutex<Cell<PhysFrame>>> = Once::new();

/// Check if the page frame allocator is currently locked.
pub fn allocator_locked() -> bool {
    PAGE_FRAME_ALLOCATOR.is_locked()
}

/// Helper function to convert a u64 address to a PhysFrame.
/// The given address is aligned up to the page size (4 KiB).
pub fn frame_from_u64(
    addr: u64,
) -> Result<PhysFrame<Size4KiB>, x86_64::structures::paging::page::AddressNotAligned> {
    let pa = PhysAddr::new(addr).align_up(PAGE_SIZE as u64);
    PhysFrame::from_start_address(pa)
}

/// Insert an available memory `region` obtained during the boot process.
pub unsafe fn insert(mut region: PhysFrameRange) {
    PHYS_LIMIT.call_once(|| {
        Mutex::new(Cell::new(
            PhysFrame::from_start_address(PhysAddr::zero()).unwrap(),
        ))
    });

    // Make sure, the first page is not inserted to avoid null pointer panics
    if region.start.start_address() == PhysAddr::zero() {
        let first_page = PhysFrame::from_start_address(PhysAddr::new(PAGE_SIZE as u64)).unwrap();
        if region.end <= first_page {
            // Region only contains the first page -> Skip insertion
            return;
        }

        region.start = first_page; // Cut first page out of region and continue
    }

    let current_limit = PHYS_LIMIT.get().unwrap().lock();
    if region.end > current_limit.get() {
        current_limit.swap(&Cell::new(region.end));
    }

    unsafe {
        free(region);
    }
}

/// Allocate `frame_count` contiguous page frames.
pub fn alloc(frame_count: usize) -> PhysFrameRange {
    PAGE_FRAME_ALLOCATOR.lock().alloc_block(frame_count)
}

/// Free a contiguous range of page `frames`.
/// Unsafe because invalid parameters may break the list allocator.
pub unsafe fn free(frames: PhysFrameRange) {
    unsafe {
        PAGE_FRAME_ALLOCATOR.lock().free_block(frames);
    }
}

/// Permanently reserve a range of page `frames`.
pub unsafe fn reserve(frames: PhysFrameRange) {
    unsafe {
        PAGE_FRAME_ALLOCATOR.lock().reserve_block(frames);
    }
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
    next: Option<&'static mut PageFrameNode>,
}

impl PageFrameNode {
    const fn new(frame_count: usize) -> Self {
        Self {
            frame_count,
            next: None,
        }
    }

    fn start(&self) -> PhysFrame {
        PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(self) as u64)).unwrap()
    }

    fn end(&self) -> PhysFrame {
        self.start() + self.frame_count as u64
    }
}

/// Manages blocks of available physical memory as a linked list
/// Since each page frame is exactly 4 KiB large, allocations are always a multiple of 4096.
struct PageFrameListAllocator {
    head: PageFrameNode,
}

impl Debug for PageFrameListAllocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let mut available: usize = 0;

        let mut current = &self.head;
        while let Some(block) = &current.next {
            writeln!(
                f,
                "Block: [0x{:x} - 0x{:x}], Frame count: [{}]",
                block.start().start_address().as_u64(),
                block.end().start_address().as_u64(),
                block.end() - block.start()
            )?;
            available += block.frame_count;

            current = current.next.as_ref().unwrap();
        }

        writeln!(
            f,
            "Available memory: [{} KiB]",
            available * PAGE_SIZE / 1024
        )?;
        write!(
            f,
            "Physical limit: [0x{:0>16x}]",
            PHYS_LIMIT
                .get()
                .unwrap()
                .lock()
                .get()
                .start_address()
                .as_u64()
        )
    }
}

impl PageFrameListAllocator {
    pub const fn new() -> Self {
        Self {
            head: PageFrameNode::new(0),
        }
    }

    /// Insert a new range of `frames`, sorted ascending by its memory address.
    unsafe fn insert(&mut self, frames: PhysFrameRange) {
        let mut new_block = PageFrameNode::new((frames.end - frames.start) as usize);
        let new_block_ptr = frames.start.start_address().as_u64() as *mut PageFrameNode;

        // Check if list is empty
        if self.head.next.is_none() {
            unsafe {
                new_block.next = self.head.next.take();
                new_block_ptr.write(new_block);
                self.head.next = Some(&mut *new_block_ptr);
            }

            return;
        }

        // List is not empty -> Search for correct position and insert new block
        let mut current = &mut self.head;
        while let Some(ref mut block) = current.next {
            if block.start().start_address() > frames.start.start_address() {
                unsafe {
                    new_block.next = current.next.take();
                    new_block_ptr.write(new_block);
                    current.next = Some(&mut *new_block_ptr);
                }

                return;
            }

            current = current.next.as_mut().unwrap()
        }

        // Insert new block at the list's end
        unsafe {
            new_block.next = None;
            new_block_ptr.write(new_block);
            current.next = Some(&mut *new_block_ptr);
        }
    }

    /// Search a block with `frame_count` contiguous page frames.
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
        None
    }

    /// Allocate a block with `frame_count` contiguous page frames.
    fn alloc_block(&mut self, frame_count: usize) -> PhysFrameRange {
        //      info!("frames: alloc_block:{} frames!", frame_count);
        match self.find_free_block(frame_count) {
            Some(block) => {
                let remaining = PhysFrameRange {
                    start: block.start() + frame_count as u64,
                    end: block.end(),
                };
                // info!("   found free block: [0x{:x} - 0x{:x}], Frame count: [{}]", block.start().start_address().as_u64(), block.end().start_address().as_u64(), block.frame_count);
                if (remaining.end - remaining.start) > 0 {
                    //   info!("   remaining frames: [{}]", remaining.end - remaining.start);
                    unsafe {
                        self.insert(remaining);
                    }
                }
                //info!("   remaining block: [0x{:x} - 0x{:x}], Frame count: [{}]", remaining.start.start_address().as_u64(), remaining.end.start_address().as_u64(), remaining.end - remaining.start);

                let ret_block = PhysFrameRange {
                    start: block.start(),
                    end: remaining.start,
                };
                //info!("   returning block: [0x{:x} - 0x{:x}], Frame count: [{}]", ret_block.start.start_address().as_u64(), ret_block.end.start_address().as_u64(), ret_block.end - ret_block.start);
                ret_block
                //                return PhysFrameRange { start: block.start(), end: remaining.start };
            }
            None => {
                info!(
                    "alloc_block: No free block found for {frame_count} frames!",
                );
                panic!("PageFrameAllocator: Out of memory!")
            }
        }
    }

    /// Free a region of `frames` consisting of at least one page frame.
    /// The block is inserted ascending by address and fused with its neighbours, if possible.
    unsafe fn free_block(&mut self, frames: PhysFrameRange) {
        let mut current = &mut self.head;
        let new_block_ptr: *mut PageFrameNode;

        // Run through list and check if fusion is possible
        while let Some(ref mut block) = current.next {

            // Check if the block to free overlaps with the current block
            if !(frames.end <= block.start() || frames.start >= block.end()) {
                panic!(
                    "free_block: Double-free or overlapping free detected!\n\
            Trying to free: [{:#x} - {:#x})\n\
            Overlaps with:  [{:#x} - {:#x})",
                    frames.start.start_address().as_u64(),
                    frames.end.start_address().as_u64(),
                    block.start().start_address().as_u64(),
                    block.end().start_address().as_u64()
                );
            }

            if frames.end == block.start() {
                // The freed memory block extends 'block' from the bottom
                let mut new_block =
                    PageFrameNode::new(block.frame_count + (frames.end - frames.start) as usize);

                unsafe {
                    new_block_ptr = frames.start.start_address().as_u64() as *mut PageFrameNode;
                    new_block.next = block.next.take();
                    new_block_ptr.write(new_block);

                    current.next = Some(&mut *new_block_ptr);
                }

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

        unsafe {
            self.insert(frames);
        }
    }

    /// Permanently reserve a frame region given by `reserved`.
    unsafe fn reserve_block(&mut self, reserved: PhysFrameRange) {
        let mut current = &mut self.head;

        // Run through list and search for free blocks, containing the reserved block
        while let Some(ref mut block) = current.next {
            if block.start() > reserved.end {
                // Block lies completely above reserved region and since the list is sorted, we can abort
                break;
            } else if block.start() < reserved.start && block.end() >= reserved.start {
                // Block starts below the reserved region
                if block.end() <= reserved.end {
                    // Block starts below and ends inside the reserved region
                    let overlapping = block.end() - reserved.start;
                    block.frame_count -= overlapping as usize;
                } else {
                    // Block starts below and ends above the reserved region
                    let below_size = reserved.start - block.start();
                    let above_size = block.end() - reserved.end;
                    block.frame_count = below_size as usize;

                    let mut above_block = PageFrameNode::new(above_size as usize);
                    let above_block_ptr =
                        reserved.end.start_address().as_u64() as *mut PageFrameNode;

                    unsafe {
                        above_block.next = block.next.take();
                        block.next = Some(&mut *above_block_ptr);
                        above_block_ptr.write(above_block);
                    }
                }
            } else if block.start() <= reserved.end && block.end() >= reserved.start {
                // Block starts within the reserved region
                if block.end() <= reserved.end {
                    // Block start within and ends within the reserved region
                    current.next = block.next.take();
                } else {
                    // Block starts within and ends above the reserved region
                    let overlapping = reserved.end - block.start();
                    let mut new_block =
                        PageFrameNode::new(block.frame_count - overlapping as usize);
                    let new_block_ptr = (block.start() + overlapping).start_address().as_u64()
                        as *mut PageFrameNode;

                    unsafe {
                        new_block.next = block.next.take();
                        new_block_ptr.write(new_block);
                        current.next = Some(&mut *new_block_ptr);
                    }
                }
            }

            current = current.next.as_mut().unwrap();
        }
    }
}
