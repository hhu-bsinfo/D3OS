use pci_types::{Bar, ConfigRegionAccess};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::{Page, PageRange};
use x86_64::structures::paging::{PageTableFlags, PhysFrame, Size4KiB};

use crate::memory::PAGE_SIZE;
use crate::{memory, process_manager};
use x86_64::{PhysAddr, VirtAddr};

use core::mem;

use alloc::slice;
use crate::memory::vma::VmaType;

pub type PageToFrameMapping = (MappedPages, PhysAddr);

#[derive(Clone, Copy, Debug)]
pub struct MappedPages {
    range: PageRange<Size4KiB>,
}

pub struct PageToFrameRange {
    mapped_pages: MappedPages,
    start_frame: PhysFrame<Size4KiB>,
}

impl PageToFrameRange {
    pub fn from_frame(page_range: PageRange<Size4KiB>, start_frame: PhysFrame<Size4KiB>) -> Self {
        let start_phys = get_physical_address(page_range.start.start_address());
        if start_phys.is_null() || (start_frame.start_address() != start_phys) || !(start_frame.start_address().is_aligned(PAGE_SIZE as u64)) {
            let null_page = Page::<Size4KiB>::containing_address(VirtAddr::zero());
            let post_page_range = Page::range(null_page, null_page);
            let post_frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::zero());

            return Self {
                mapped_pages: MappedPages { range: post_page_range },
                start_frame: post_frame,
            };
        }

        Self {
            mapped_pages: MappedPages { range: page_range },
            start_frame,
        }
    }

    pub fn from_phy(page_range: PageRange<Size4KiB>, start: PhysAddr) -> Self {
        let start_phys = get_physical_address(page_range.start.start_address());
        if start_phys.is_null() || (start != start_phys) || !(start.is_aligned(PAGE_SIZE as u64)) {
            let null_page = Page::<Size4KiB>::containing_address(VirtAddr::zero());
            let post_page_range = Page::range(null_page, null_page);
            let post_frame = PhysFrame::<Size4KiB>::containing_address(PhysAddr::zero());

            return Self {
                mapped_pages: MappedPages { range: post_page_range },
                start_frame: post_frame,
            };
        }

        Self {
            mapped_pages: MappedPages { range: page_range },
            start_frame: unsafe { PhysFrame::<Size4KiB>::from_start_address_unchecked(start) },
        }
    }

    pub fn is_valid(&self) -> bool {
        self.mapped_pages.non_zero() & !self.start_frame.start_address().is_null()
    }

    pub fn fetch_in_frame(&self) -> Result<(MappedPages, PhysFrame<Size4KiB>), &'static str> {
        if !self.is_valid() {
            return Err("Not a valid mapping -> fetching not possible");
        }

        Ok((self.mapped_pages, self.start_frame))
    }

    pub fn fetch_in_addr(&self) -> Result<(MappedPages, PhysAddr), &'static str> {
        if !self.is_valid() {
            return Err("Not a valid mapping -> fetching not possible");
        }

        Ok((self.mapped_pages, self.start_frame.start_address()))
    }
}

// wrapper type around page range, to mark mapped allocated pages
impl MappedPages {
    pub fn from(page_range: PageRange<Size4KiB>) -> Self {
        Self { range: page_range }
    }

    // the function handling is completly adapted from Theseus
    // remove the trait bound FromBytes, and allow T to be any type
    pub fn as_type_mut<T>(&mut self, byte_offset: usize) -> Result<&mut T, &'static str> {
        let size = mem::size_of::<T>();
        if byte_offset % mem::align_of::<T>() != 0 {
            return Err("Not aligned properly");
        }

        // assuming not out of bound
        let start_vaddr = start_page_as_ptr::<u8>(self.range.start);
        let end_vaddr = start_page_as_ptr::<u8>(self.range.end);

        let end_bound_vaddr = unsafe { start_vaddr.add(byte_offset + size) };

        if end_vaddr < end_bound_vaddr {
            return Err("Doesn't fit within pages");
        }

        let t = unsafe { &mut *(start_vaddr.add(byte_offset) as *mut T) };

        Ok(t)
    }

    pub fn as_type<T>(&self, byte_offset: usize) -> Result<&T, &'static str> {
        let size = mem::size_of::<T>();
        if byte_offset % mem::align_of::<T>() != 0 {
            return Err("Not aligned properly");
        }

        // assuming not out of bound
        let start_vaddr = start_page_as_ptr::<u8>(self.range.start);
        let end_vaddr = start_page_as_ptr::<u8>(self.range.end);

        let end_bound_vaddr = unsafe { start_vaddr.add(byte_offset + size) };

        if end_vaddr < end_bound_vaddr {
            return Err("Doesn't fit within pages");
        }

        let t = unsafe { &*(start_vaddr.add(byte_offset) as *const T) };

        Ok(t)
    }

    // the function handling is completly adapted from Theseus
    pub fn as_slice<T>(&self, byte_offset: usize, length: usize) -> Result<&[T], &'static str> {
        let size_in_bytes = length.checked_mul(mem::size_of::<T>()).ok_or("overflow")?;

        if byte_offset % mem::align_of::<T>() != 0 {
            return Err("not aligned properly");
        }

        let start_vaddr = start_page_as_ptr::<u8>(self.range.start);
        let end_vaddr = start_page_as_ptr::<u8>(self.range.end);

        let end_bound_vaddr = unsafe { start_vaddr.add(byte_offset + size_in_bytes) };

        if end_vaddr < end_bound_vaddr {
            return Err("Doesn't fit within pages");
        }

        let start_data_vaddr = unsafe { start_vaddr.add(byte_offset) };

        let slc = unsafe { slice::from_raw_parts(start_data_vaddr as *const T, length) };

        Ok(slc)
    }

    pub fn as_slice_mut<T>(&mut self, byte_offset: usize, length: usize) -> Result<&mut [T], &'static str> {
        let size_in_bytes = length.checked_mul(mem::size_of::<T>()).ok_or("overflow")?;

        if byte_offset % mem::align_of::<T>() != 0 {
            return Err("not aligned properly");
        }

        let start_vaddr = start_page_as_ptr::<u8>(self.range.start);
        let end_vaddr = start_page_as_ptr::<u8>(self.range.end);

        let end_bound_vaddr: *const u8 = unsafe { start_vaddr.add(byte_offset + size_in_bytes) };

        if end_vaddr < end_bound_vaddr {
            return Err("Doesn't fit within pages");
        }

        let start_data_vaddr = unsafe { start_vaddr.add(byte_offset) };

        let slc = unsafe { slice::from_raw_parts_mut(start_data_vaddr as *mut T, length) };

        Ok(slc)
    }

    pub fn offset_of_address(&self, addr: VirtAddr) -> Option<usize> {
        let start_vaddr = start_page_as_ptr::<u8>(self.range.start);
        let end_vaddr = start_page_as_ptr::<u8>(self.range.end);
        let target_vaddr = addr.as_ptr::<u8>();

        if target_vaddr >= end_vaddr {
            return None;
        }

        let offset = unsafe { target_vaddr.offset_from(start_vaddr) };

        if offset < 0 {
            return None;
        }

        Some(offset as usize)
    }

    pub fn non_zero(&self) -> bool {
        !self.range.is_empty()
    }

    pub fn into_range(&self) -> PageRange<Size4KiB> {
        self.range
    }
}

pub fn mapped_pages_from_frames(frame_range: PhysFrameRange) -> PageRange<Size4KiB> {
    let v1 = VirtAddr::new(frame_range.start.start_address().as_u64());
    let v2 = VirtAddr::new(frame_range.end.start_address().as_u64());

    let start = Page::<Size4KiB>::from_start_address(v1).unwrap();
    let end_exclusive = Page::<Size4KiB>::from_start_address(v2).unwrap();

    PageRange::<Size4KiB> {
        start: start,
        end: end_exclusive,
    }
}

pub fn v_address_align_up(start: VirtAddr, size: usize) -> VirtAddr {
    let offset = start + size as u64;
    offset.align_up(PAGE_SIZE as u64)
}

pub fn pages_required(bytes: usize) -> usize {
    (bytes + PAGE_SIZE - 1) / PAGE_SIZE
}

pub fn start_page_as_mut_ptr<T>(page: Page<Size4KiB>) -> *mut T {
    page.start_address().as_mut_ptr::<T>()
}

pub fn start_page_as_ptr<T>(page: Page<Size4KiB>) -> *const T {
    page.start_address().as_ptr::<T>()
}

pub fn get_memory_of(frame: PhysFrame) -> PhysAddr {
    frame.start_address()
}

pub fn fill_pages(fill_value: u8, for_page_raw: *mut u8, len: usize) {
    let slc: &mut [u8] = unsafe { slice::from_raw_parts_mut(for_page_raw, len) };
    slc.fill(fill_value);
}

pub fn copy_pages(from_slice: &[u8], tgt_page_raw: *mut u8, len: usize) {
    let slc_sub: &mut [u8] = unsafe { slice::from_raw_parts_mut(tgt_page_raw, len) };
    slc_sub.copy_from_slice(from_slice);
}

pub fn set_dma_flags(page_range: PageRange<Size4KiB>) {
    process_manager()
        .write()
        .current_process()
        .virtual_address_space
        .set_flags(page_range, PageTableFlags::NO_EXECUTE | PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
}

pub fn get_physical_address(addr: VirtAddr) -> PhysAddr {
    process_manager().read().current_process().virtual_address_space.get_phys(addr.as_u64()).unwrap_or(PhysAddr::zero())
}

pub fn pci_map_bar_mem(bar: Bar, tag: &str) -> MappedPages {
    let (address, size) = bar.unwrap_mem();
    let end_address = address + size;
    let process = process_manager().write().current_process();
    let pages = process.virtual_address_space.kernel_map_devm_identity(
        address as u64,
        end_address as u64,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_CACHE, 
        VmaType::DeviceMemory,
        tag
    );
    MappedPages::from(pages)
}

pub fn create_cont_mapping_with_dma_flags(frame_count: usize) -> Result<PageToFrameRange, &'static str> {
    let memory = memory::alloc_frames(frame_count);
    let page_range = mapped_pages_from_frames(memory);
    set_dma_flags(page_range);

    let pagetoframe = PageToFrameRange::from_frame(page_range, memory.start);

    Ok(pagetoframe)
}
