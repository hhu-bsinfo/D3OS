/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: sys_vmem                                                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: All system calls related to virtual memory management.          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland & Michael Schoettner, 24.5.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use multiboot2::FramebufferTag;
use spin::once::Once;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame};
use graphic::lfb::FramebufferInfo;
use crate::memory::vma::VmaType;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process_manager;
use syscall::return_vals::Errno;

static FB_INFO: Once<FramebufferInfo> = Once::new();

pub fn init_fb_info(tag: &FramebufferTag) {
    FB_INFO.call_once(|| {
        let start = PhysAddr::new(tag.address());

        FramebufferInfo {
            addr: start.as_u64(),
            width: tag.width(),
            height: tag.height(),
            pitch: tag.pitch(),
            bpp: tag.bpp()
        }
    });
}

/// Map memory to a process.
///
/// This just sets up the VMA, no page tables are created yet.
/// This happens later on on page faults.
pub extern "sysv64" fn sys_map_memory(start: usize, size: usize) -> isize {
    let process = process_manager().read().current_process();

    let start_addr = VirtAddr::new(start.try_into().unwrap());
    let start_page = Page::containing_address(start_addr);
    let num_pages = size.div_ceil(PAGE_SIZE);

    let vma = process.virtual_address_space.alloc_vma(
        Some(start_page),
        num_pages as u64,
        MemorySpace::User,
        VmaType::Heap,
        "heap",
    );
    if vma.is_none() {
        Errno::EUNKN as isize
    } else {
        0
    }
}

pub extern "sysv64" fn sys_map_frame_buffer(fb_info_user: *mut FramebufferInfo) -> isize {
    let process = process_manager().read().current_process();

    let fb_info = FB_INFO.get().unwrap();
    let size = fb_info.height * fb_info.pitch;
    let num_pages = size.div_ceil(PAGE_SIZE as u32) as u64;
    let start_frame = PhysFrame::from_start_address(PhysAddr::new(fb_info.addr)).unwrap();
    let end_frame = start_frame + num_pages;

    let vma = process.virtual_address_space.alloc_vma(
        None,
        num_pages,
        MemorySpace::User,
        VmaType::DeviceMemory,
        "framebuffer",
    );
    if vma.is_none() {
        return Errno::EUNKN as isize
    }

    let res = process.virtual_address_space.map_pfr_for_vma(
        vma.as_ref().unwrap(),
        PhysFrameRange{ start: start_frame, end: end_frame },
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE | PageTableFlags::NO_CACHE);
    if res.is_err() {
        return Errno::EUNKN as isize;
    }

    unsafe {
        let fb_info_user = &mut *fb_info_user;
        fb_info_user.addr = vma.unwrap().start().as_u64();
        fb_info_user.width = fb_info.width;
        fb_info_user.height = fb_info.height;
        fb_info_user.pitch = fb_info.pitch;
        fb_info_user.bpp = fb_info.bpp;
    }

    0
}