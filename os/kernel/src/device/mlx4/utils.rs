use pci_types::Bar;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{PageTableFlags, PhysFrame, Size4KiB};

use crate::memory::PAGE_SIZE;
use crate::process_manager;
use x86_64::{PhysAddr, VirtAddr};

use core::mem;

use crate::memory::vma::VmaType;
use alloc::slice;
use log::error;

// wrapper type around page range, to mark mapped allocated pages
#[derive(Clone, Copy, Debug)]
pub struct MappedPages {
    range: PageRange<Size4KiB>,
    start_frame: PhysFrame<Size4KiB>,
}

impl MappedPages {
    pub unsafe fn from_identity_unchecked(page_range: PageRange<Size4KiB>) -> Self {
        let phys_addr = PhysAddr::new(page_range.start.start_address().as_u64());
        let phys_frame = unsafe { PhysFrame::from_start_address_unchecked(phys_addr) };
        Self { range: page_range, start_frame: phys_frame }
    }

    pub fn start_frame(&self) -> PhysFrame<Size4KiB> {
        self.start_frame
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.range.start.start_address().as_ptr(), self.range.size() as usize) }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.range.start.start_address().as_mut_ptr(), self.range.size() as usize) }
    }

    #[inline]
    fn check_align_bounds(&self, offset: usize, size: usize, algin: usize) -> Result<*const u8, &'static str> {
        if offset % algin != 0 {
            return Err("Not aligned properly");
        }

        let start_vaddr = self.range.start.start_address().as_ptr::<u8>();
        let end_vaddr = self.range.end.start_address().as_ptr::<u8>();

        let start_bound_vaddr = unsafe { start_vaddr.add(offset) };
        let end_bound_vaddr = unsafe { start_vaddr.add(offset + size) };

        if end_bound_vaddr > end_vaddr {
            error!("{:?}", self);
            error!("Out of bounds: offset = {:x}, size = {:x}", offset, size);
            return Err("Doesn't fit within pages")
        }

        Ok(start_bound_vaddr)
    }

    // remove the trait bound FromBytes, and allow T to be any type
    pub fn as_type_mut<T>(&mut self, byte_offset: usize) -> Result<&mut T, &'static str> {
        let ptr = self.check_align_bounds(byte_offset, size_of::<T>(), align_of::<T>())?;
        let t = unsafe { &mut *(ptr as *mut T) };
        Ok(t)
    }

    pub fn as_type<T>(&self, byte_offset: usize) -> Result<&T, &'static str> {
        let ptr = self.check_align_bounds(byte_offset, size_of::<T>(), align_of::<T>())?;
        let t = unsafe { &*(ptr as *const T) };
        Ok(t)
    }

    pub fn as_slice<T>(&self, byte_offset: usize, length: usize) -> Result<&[T], &'static str> {
        let size_in_bytes = length.checked_mul(mem::size_of::<T>()).ok_or("overflow")?;
        let ptr = self.check_align_bounds(byte_offset, size_in_bytes, align_of::<T>())?;
        let slc = unsafe { slice::from_raw_parts(ptr as *const T, length) };
        Ok(slc)
    }

    pub fn as_slice_mut<T>(&mut self, byte_offset: usize, length: usize) -> Result<&mut [T], &'static str> {
        let size_in_bytes = length.checked_mul(mem::size_of::<T>()).ok_or("overflow")?;
        let ptr = self.check_align_bounds(byte_offset, size_in_bytes, align_of::<T>())?;
        let slc = unsafe { slice::from_raw_parts_mut(ptr as *mut T, length) };
        Ok(slc)
    }

    pub fn offset_of_address(&self, addr: VirtAddr) -> Option<usize> {
        let start_vaddr = self.range.start.start_address().as_ptr::<u8>();
        let end_vaddr = self.range.end.start_address().as_ptr::<u8>();
        let target_vaddr = addr.as_ptr::<u8>();

        if target_vaddr < start_vaddr || end_vaddr <= target_vaddr {
            return None;
        }

        let offset = unsafe { target_vaddr.offset_from(start_vaddr) };

        Some(offset as usize)
    }

    pub fn non_zero(&self) -> bool {
        !self.range.is_empty()
    }

    pub fn page_range(&self) -> PageRange<Size4KiB> {
        self.range
    }
}

pub fn pci_map_bar_mem(bar: Bar, tag: &str) -> Option<MappedPages> {
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

    // SAFETY: identity mapped
    Some(unsafe { MappedPages::from_identity_unchecked(pages) })
}

pub fn create_cont_mapping_with_dma_flags(frame_count: usize) -> Result<MappedPages, &'static str> {
    if frame_count == 0 {
        return Err("frame_count must not be zero");
    }

    let process = process_manager().read().current_process();
    let page_range = process.virtual_address_space.kernel_alloc_map_identity(
        frame_count as u64,
        PageTableFlags::NO_EXECUTE | PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        VmaType::DeviceMemory,
        "mlx_dma"
    );

    if page_range.is_empty() {
        return Err("Failed to allocate pages");
    }

    // SAFETY: identity mapped
    Ok(unsafe { MappedPages::from_identity_unchecked(page_range) })
}
