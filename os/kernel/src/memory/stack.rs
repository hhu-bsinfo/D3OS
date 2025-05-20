/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: stack                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Memory for a stack (user or kernel). The stack will be accessed within  ║
   ║ the kernel through a Vec and thus a Allocator is required.              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, HHU, 20.05.2025            ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec::Vec;
use uefi_raw::protocol::scsi::ScsiIoScsiRequestPacket;
use crate::memory::frames::phys_limit;
use crate::memory::{PAGE_SIZE, frames};
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
use log::info;
use x86_64::PhysAddr;
use x86_64::structures::paging::PhysFrame;
use x86_64::structures::paging::frame::{self, PhysFrameRange};


#[derive(Default)]
pub struct StackAllocator {
    pid: usize,  // process id the stack belongs to
    tid: usize,  // thread id the stack belongs to
    phys_start: AtomicUsize, // physical start address of the first frame used for the stack
    phys_end: AtomicUsize, // physical end address of the last frame used for the stack
}

impl StackAllocator {
    pub fn new(pid: usize, tid: usize) -> Self {
        StackAllocator {
            pid,
            tid,
            phys_start: AtomicUsize::new(0),
            phys_end: AtomicUsize::new(0),
        }
    }

    pub fn get_tid(&self) -> usize {
        self.tid
    }

    pub fn get_pid(&self) -> usize {
        self.pid
    }

    pub fn get_frame_range(&self) -> PhysFrameRange {

        let start = self.phys_start.load(Ordering::SeqCst);
        let end = self.phys_end.load(Ordering::SeqCst);

        let start_frame = PhysFrame::from_start_address(PhysAddr::new(start as u64)).unwrap();
        let end_frame = PhysFrame::from_start_address(PhysAddr::new(end as u64)).unwrap();

        PhysFrameRange { start: start_frame, end: end_frame }
    }
}

unsafe impl Allocator for StackAllocator {

    /// 'Allocate is called only once during stack creating.
    /// User stacks are resized transparently if a page fault occurs.
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if PAGE_SIZE % layout.align() != 0 {
            return Err(AllocError);
        }
        let frame_count = if layout.size() % PAGE_SIZE == 0 {
            layout.size() / PAGE_SIZE
        } else {
            (layout.size() / PAGE_SIZE) + 1
        };
        let frames = frames::alloc(frame_count);

        self.phys_start.store(frames.start.start_address().as_u64() as usize, Ordering::SeqCst);
        self.phys_end.store(frames.end.start_address().as_u64() as usize, Ordering::SeqCst);

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(frames.start.start_address().as_u64() as *mut u8).unwrap(),
            (frames.end - frames.start) as usize * PAGE_SIZE,
        ))
    }

    /// Deallocate is called when a thread terminates.
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        info!("deallocate() called for stack of pid = {}, tid = {}", self.pid, self.tid);

        // Ignore virtual addresses
        if (ptr.as_ptr() as usize) < phys_limit().start_address().as_u64() as usize {
            assert_eq!(PAGE_SIZE % layout.align(), 0);
            assert_eq!(layout.size() % PAGE_SIZE, 0);

            let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64)).unwrap();
            unsafe {
                frames::free(PhysFrameRange {
                    start,
                    end: start + (layout.size() / PAGE_SIZE) as u64,
                });
            }
        }
    }
}
