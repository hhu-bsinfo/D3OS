/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: kstack                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Allocator for the kernel stack of a thread. Allocates only page frames  ║
   ║ because kernel maps all frames 1:1, so page table entries are already   ║
   ║ initialized.                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 02.03.2025                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::{self, PhysFrameRange};
use x86_64::structures::paging::PhysFrame;
use crate::memory::{PAGE_SIZE, frames};
use crate::memory::frames::phys_limit;
use log::info;


#[derive(Default)]
pub struct StackAllocator {}


unsafe impl Allocator for StackAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if PAGE_SIZE % layout.align() != 0 {
            return Err(AllocError);
        }

        let frame_count = if layout.size() % PAGE_SIZE == 0 { layout.size() / PAGE_SIZE } else { (layout.size() / PAGE_SIZE) + 1 };
        let frames = frames::alloc(frame_count);
        Ok(NonNull::slice_from_raw_parts(NonNull::new(frames.start.start_address().as_u64() as *mut u8).unwrap(), (frames.end - frames.start) as usize * PAGE_SIZE))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Ignore virtual addresses
        if (ptr.as_ptr() as usize) < phys_limit().start_address().as_u64() as usize {
            assert_eq!(PAGE_SIZE % layout.align(), 0);
            assert_eq!(layout.size() % PAGE_SIZE, 0);

            let start = PhysFrame::from_start_address(PhysAddr::new(ptr.as_ptr() as u64)).unwrap();
            unsafe { frames::free(PhysFrameRange { start, end: start + (layout.size() / PAGE_SIZE) as u64 }); }
        }
    }
}

