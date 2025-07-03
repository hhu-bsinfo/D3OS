/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: virtual memory management                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Functions related to a virtual memory management of a process address   ║
   ║ space. This includes managing virtual memory areas, allocating frames   ║
   ║ for full or partial vmas, as well as creating page mappings.            ║
   ║                                                                         ║
   ║ Public convenience functions:                                           ║
   ║   - map_devmem_identity       map physical device memory in kernel space║
   ║                               (identity mapped) and allocate a vma      ║
   ║   - alloc_map_identity        allocate page frames in kernel space and  ║
   ║                               a vma and create a identity mapping       ║
   ║   - user_alloc_map_full       create vma for pages, allocate and map it ║
   ║                               in user space.                            ║
   ║                                                                         ║
   ║ Public functions:                                                       ║
   ║   - alloc_vma                 allocate a page range in an address space ║
   ║   - alloc_pfr_for_vma         allocate pf range for full vma            ║
   ║   - alloc_pfr_for_partial_vma alloc pf range for a subrange of a vma    ║
   ║   - map_pfr_for_vma           map pf range for full vma                 ║
   ║   - map_pfr_for_partial_vma   map pf range for subrange of a vma        ║
   ║   - map_partial_vma           map a sub page range of a vma by          ║
   ║                               allocating frames as needed               ║
   ║                                                                         ║
   ║   - clone_address_space       used for process creation                 ║
   ║   - create_kernel_address_space   used for process creation             ║
   ║   - iter_vmas                 Iterate over all VMAs                     ║
   ║   - dump                      dump all VMAs of an address space         ║
   ║   - page_table_address        get root page table address               ║
   ║   - set_flags                 set page table flags                      ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland and Michael Schoettner                           ║
   ║         Univ. Duesseldorf, 26.05.2025                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

///
/// This module provides functions to manage virtual memory areas (VMAs) in
/// a process address space. Below is a description of steps for typical
/// memory allocations.
///
///  Map device memory:
///     => map_devmem_identity
///  
/// User stack:
///     1. alloc_vma
///     2. alloc_pfr_for_partial_vma
///     3. map_partial_vma
///
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ops::Deref;
use log::info;
use spin::RwLock;

use x86_64::PhysAddr;
use x86_64::VirtAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags};

use crate::cpu;
use crate::memory::frames;
use crate::memory::frames::phys_limit;
use crate::memory::pages;
use crate::memory::pages::Paging;
use crate::memory::vma::{VirtualMemoryArea, VmaType};
use crate::memory::{MemorySpace, PAGE_SIZE};


/// Clone address space. Used during process creation.
pub fn clone_address_space(other: &VirtualAddressSpace) -> Arc<Paging> {
    Arc::new(Paging::from_other(&other.page_tables()))
}

/// Create kernel address space. Used during process creation.
pub fn create_kernel_address_space() -> Arc<Paging> {
    let address_space = Paging::new(4);
    // map all physical addresses 1:1
    let max_phys_addr = phys_limit().start_address();
    let range = PageRange {
        start: Page::containing_address(VirtAddr::zero()),
        end: Page::containing_address(VirtAddr::new(max_phys_addr.as_u64())),
    };

    address_space.map(range, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
    Arc::new(address_space)
}

/// Return the last useable virtual address in canonical form
fn last_usable_virtual_address() -> u64 {
    let virtual_bits = cpu().linear_address_bits();
    (1u64 << (virtual_bits - 1)) - 1
}

pub struct VmaIterator {
    vmas: Vec<Arc<VirtualMemoryArea>>,
    index: usize,
}

impl VmaIterator {
    pub fn new(vmas: Vec<Arc<VirtualMemoryArea>>) -> Self {
        Self { vmas, index: 0 }
    }
}

impl Iterator for VmaIterator {
    type Item = Arc<VirtualMemoryArea>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.vmas.len() {
            let vma = Arc::clone(&self.vmas[self.index]);
            self.index += 1;
            Some(vma)
        } else {
            None
        }
    }
}

/// All data related to a virtual address space of a process.
pub struct VirtualAddressSpace {
    virtual_memory_areas: RwLock<Vec<Arc<VirtualMemoryArea>>>,
    page_tables: Arc<Paging>,
    first_usable_user_addr: VirtAddr,
    last_usable_user_addr: VirtAddr,
}

impl VirtualAddressSpace {
    /// Initialize a new virtual address space with the given `page_tables`.
    pub fn new(page_tables: Arc<Paging>) -> Self {
        let first_usable_user_addr = VirtAddr::new(crate::consts::USER_SPACE_START as u64);
        let last_usable_user_addr: VirtAddr = VirtAddr::new(last_usable_virtual_address());
        info!(
            "VirtualAddressSpace: first usable user address: 0x{:x}, last usable user address: 0x{:x}",
            first_usable_user_addr.as_u64(),
            last_usable_user_addr.as_u64()
        );

        Self {
            page_tables,
            virtual_memory_areas: RwLock::new(Vec::new()),
            first_usable_user_addr,
            last_usable_user_addr,
        }
    }

    pub fn load_address_space(&self) {
        self.page_tables.load();
    }

    pub fn page_tables(&self) -> Arc<Paging> {
        Arc::clone(&self.page_tables)
    }

    /// Tries to allocate a virtual memory region for `num_pages` pages for the given `space`, `typ`, and `tag` in the address space `self`. \
    /// If `start_page` is `Some` the allocator tries to allocate the vma from the given page otherwise it will allocate from any free page. \
    /// No frames are allocated and no mappings are created in the page tables. \
    /// Returns the new [`VirtualMemoryArea`] if successful, otherwise `None`.
    pub fn alloc_vma(
        &self, start_page: Option<Page>, num_pages: u64, vma_space: MemorySpace, vma_type: VmaType, vma_tag: &str,
    ) -> Option<Arc<VirtualMemoryArea>> {
        match start_page {
            Some(start_page) => self.alloc_at(start_page, num_pages, vma_space, vma_type, vma_tag),
            None => self.alloc(num_pages, vma_space, vma_type, vma_tag),
        }
    }

    /// Tries to allocate a frame range for the full `vma`. \
    /// Returns the allocated [`PhysFrameRange`] if successful, otherwise `None`.
    pub fn alloc_pf_for_vma(&self, vma: &VirtualMemoryArea) -> Option<PhysFrameRange> {
        Some(frames::alloc(vma.range.len() as usize))
    }

    /// Tries to allocate a frame range for the given `page_range` which must be within the given `vma`. \
    /// Returns the allocated [`PhysFrameRange`] if successful, otherwise `None`.
    pub fn alloc_pfr_for_partial_vma(&self, vma: &VirtualMemoryArea, page_range: PageRange) -> Option<PhysFrameRange> {
        if page_range.start < vma.range.start || page_range.end > vma.range.end {
            return None;
        }
        Some(frames::alloc(page_range.len() as usize))
    }

    /// Map `frame_range` for the full page range of the given `vma`. \
    /// The mapping will use the given `flags` for the page table entries.
    pub fn map_pfr_for_vma(&self, vma: &VirtualMemoryArea, frame_range: PhysFrameRange, mut flags: PageTableFlags) -> Result<(), i64> {
        // Check if the number of frames is identical with the number of pages of the vma
        let num_frames = frame_range.end - frame_range.start;
        let num_pages = vma.range.end - vma.range.start;
        if num_frames != num_pages {
            return Err(-1);
        }

        // Check if the flags are consistent with the vma
        flags = vma.check_and_enforce_consistency(flags);
        flags |= PageTableFlags::PRESENT;

        // Do the mapping
        self.page_tables.map_physical(frame_range, vma.range, vma.space, flags);
        Ok(())
    }

    /// Map `frame_range` for the given page range which must be witin the given `vma`. \
    /// The mapping will use the given already allocated frames and the `flags` for the page table entries.
    pub fn map_pfr_for_partial_vma(
        &self, vma: &VirtualMemoryArea, frame_range: PhysFrameRange, page_range: PageRange, mut flags: PageTableFlags,
    ) -> Result<(), i64> {
        // Check if the number of frames of the `frame_range` is identical with the number of pages of `page_range`
        let num_frames = frame_range.end - frame_range.start;
        let num_pages = vma.range.end - vma.range.start;
        if num_frames != num_pages {
            return Err(-1);
        }

        // Check if the flags are consistent with the vma
        flags = vma.check_and_enforce_consistency(flags);
        flags |= PageTableFlags::PRESENT;

        // Check if `page_range` is within the VMA range
        if page_range.start < vma.range.start || page_range.end > vma.range.end {
            return Err(-1);
        }

        // Do the mapping
        self.page_tables.map_physical(frame_range, page_range, vma.space, flags);

        Ok(())
    }

    /// Allocates a virtual memory region for `num_pages` pages, starting from `first_page` \
    /// for the given `space`, `typ`, and `tag` in the address space `self`. \
    /// No mappings are created in the page tables. \
    /// Returns the new [`VirtualMemoryArea`] if successful, otherwise `None`.
    fn alloc_at(&self, first_page: Page, num_pages: u64, vma_space: MemorySpace, vma_type: VmaType, vma_tag_str: &str) -> Option<Arc<VirtualMemoryArea>> {
        let start_addr = first_page.start_address();

        let end_page = first_page + num_pages;
        let end_addr = end_page.start_address(); // still safe, since end is exclusive

        // Bounds check against usable user address range
        if vma_space == MemorySpace::User {
            if start_addr < self.first_usable_user_addr || end_addr > self.last_usable_user_addr {
                return None;
            }
        // Bounds check against usable kernel address range
        } else if end_addr > self.last_usable_user_addr {
            return None;
        }

        // Create new VMA
        let vma_range = PageRange {
            start: first_page,
            end: first_page + num_pages,
        };
        let new_vma = Arc::new(VirtualMemoryArea::new_with_tag(vma_space, vma_range, vma_type, vma_tag_str));

        // Check for overlap with existing VMAs
        let mut vmas = self.virtual_memory_areas.write();
        vmas.sort_by(|a, b| a.range.start.cmp(&b.range.start));
        for vma in vmas.iter() {
            // Check for overlap with existing VMAs
            if vma.overlaps_with(&new_vma) {
                return None;
            }
        }

        // No overlap, add new VMA
        vmas.push(Arc::clone(&new_vma));
        Some(new_vma)
    }

    /// Allocates a virtual memory region for `num_pages` pages (starting from any free page) \
    /// for the given `space`, `typ` and `tag` in the address space `self`. \
    /// No mappings are created in the page tables. \
    /// Returns the new [`VirtualMemoryArea`] if successful, otherwise `None`.
    fn alloc(&self, num_pages: u64, vma_space: MemorySpace, vma_type: VmaType, vma_tag: &str) -> Option<Arc<VirtualMemoryArea>> {
        let mut vmas = self.virtual_memory_areas.write();
        vmas.sort_by(|a, b| a.range.start.cmp(&b.range.start));

        let requested_region_size = num_pages * PAGE_SIZE as u64;

        // Start searching from first usable user address
        let mut current_addr = self.first_usable_user_addr;
        for vma in vmas.iter() {
            let gap_start = current_addr;
            let gap_end = vma.range.start.start_address();

            if gap_end > gap_start {
                let gap_size = gap_end.as_u64() - gap_start.as_u64();

                if gap_size >= requested_region_size {
                    let candidate_page = Page::containing_address(gap_start);
                    drop(vmas); // release lock before recursive call
                    return self.alloc_at(candidate_page, num_pages, vma_space, vma_type, vma_tag);
                }
            }

            // Move to end of current VMA
            current_addr = vma.range.end.start_address();
        }

        // Try allocating after last VMA
        let last_addr = current_addr;
        let available = self.last_usable_user_addr.as_u64().saturating_sub(last_addr.as_u64());

        if available >= requested_region_size {
            let candidate_page = Page::containing_address(last_addr);
            return self.alloc_at(candidate_page, num_pages, vma_space, vma_type, vma_tag);
        }

        None // No space found
    }

    /// Iterate over all virtual memory areas in this address space.
    pub fn iter_vmas(&self) -> VmaIterator {
        let vmas = self.virtual_memory_areas.read().clone();
        VmaIterator::new(vmas)
    }

    /// Map the sub `page_range` of the given `vma` by allocating frames as needed.
    pub fn map_partial_vma(&self, vma: &VirtualMemoryArea, page_range: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let areas = self.virtual_memory_areas.read();
        areas.iter().find(|area| (**area).deref() == vma).expect("tried to map a non-existent VMA!");
        assert!(page_range.start.start_address() >= vma.start());
        assert!(page_range.end.start_address() <= vma.end());
        self.page_tables.map(page_range, space, flags);
    }

    /// Set page table `flags` for the give page range `pages`  
    pub fn set_flags(&self, pages: PageRange, flags: PageTableFlags) {
        self.page_tables.set_flags(pages, flags);
    }

    /// Get physical address of root page table
    pub fn page_table_address(&self) -> PhysAddr {
        self.page_tables.page_table_address()
    }

    /// Dump all virtual memory areas of this address space
    pub fn dump(&self, pid: usize) {
        info!("VMAs of process [{pid}]");
        let areas = self.virtual_memory_areas.read();
        for area in areas.iter() {
            info!("{area:?}");
        }
    }

    /// Helper function to align an address up to the next page boundary.
    fn align_up(addr: u64) -> u64 {
        let ps = PAGE_SIZE as u64;
        (addr + ps - 1) & !(ps - 1)
    }

    /// Map the given page-frame range [`start_phys_addr`, `end_phys_addr`) - identity mapped in kernel space. No page frames are allocated! \
    /// `start_phys_addr` must be page aligned. \
    /// `end_phys_addr` must be greater than `start_phys_addr` but no need to be page aligned. If it is not page aligned, it will be aligned up. \
    /// A vma ist created using the parameters `typ` and `tag`.
    pub fn map_devmem_identity(&self, start_phys_addr: u64, end_phys_addr: u64, flags: PageTableFlags, typ: VmaType, tag: &str) -> Page {
        assert!(end_phys_addr > start_phys_addr, "'end_phys_addr' must be larger than 'start_phys_addr'");

        // Calc page frame range (nneded for mapping))
        let start_page_frame = frames::frame_from_u64(start_phys_addr).expect("start_phys_addr is not page aligned");
        let end_page_frame = frames::frame_from_u64(Self::align_up(end_phys_addr)).expect("end_phys_addr is not page aligned");
        let pfr = PhysFrameRange {
            start: start_page_frame,
            end: end_page_frame,
        };

        // Calc page range and alloc vma
        let start_page_addr = pages::page_from_u64(start_phys_addr).expect("start_phys_addr is not page aligned");
        let end_page_addr = pages::page_from_u64(Self::align_up(end_phys_addr)).expect("end_phys_addr is not page aligned");
        let pr = PageRange {
            start: start_page_addr,
            end: end_page_addr,
        };
        let vma = self
            .alloc_vma(Some(start_page_addr), pr.len() as u64, MemorySpace::Kernel, typ, tag)
            .expect("alloc_vma failed");

        // Now we do the mapping
        self.map_pfr_for_vma(&vma, pfr, flags).expect("map_pfr_for_vma failed in map_devmem_identity");

        pr.start
    }

    /// Alloc `num_pf` page frames, en bloc, identity mapped in kernel space.
    /// A vma ist created using the parameters `typ` and `tag`.
    pub fn alloc_map_identity(&self, num_pf: u64, flags: PageTableFlags, typ: VmaType, tag: &str) -> PageRange {
        // Alloc page frame range
        let pfr = frames::alloc(num_pf as usize);

        // Create page from pfr.start
        let start_page = pages::page_from_u64(pfr.start.start_address().as_u64()).expect("pfr.start is not page aligned");

        let vma = self
            .alloc_vma(Some(start_page), pfr.len() as u64, MemorySpace::Kernel, typ, tag)
            .expect("alloc_vma failed");

        // Now we do the mapping
        self.map_pfr_for_vma(&vma, pfr, flags).expect("map_pfr_for_vma failed");

        PageRange {
            start: start_page,
            end: start_page + num_pf,
        }
    }

    /// Tries to allocate a virtual memory region for `num_pages` pages for `MemorySpace::User`, `typ`, and `tag` in the address space `self`. \
    /// If `start_page` is `Some` the allocator tries to allocate the vma from the given page otherwise it will allocate from any free page. \
    /// No frames are allocated and no mappings are created in the page tables. \
    /// Returns the new [`VirtualMemoryArea`] if successful, otherwise `None`.
    pub fn user_alloc_map_full(&self, start_page: Option<Page>, num_pages: u64, vma_type: VmaType, vma_tag: &str) -> Option<Arc<VirtualMemoryArea>> {
        info!(
            "user_alloc_map_full: start_page: {:?}, num_pages: {}, vma_type: {:?}, vma_tag: {}",
            start_page, num_pages, vma_type, vma_tag
        );
        let vma = self.alloc_vma(start_page, num_pages, MemorySpace::User, vma_type, vma_tag);
        if vma.is_none() {
            return None;
        }
        let vma = vma.unwrap();

        self.page_tables.map(
            vma.range,
            MemorySpace::User,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );

        Some(vma)
    }


    /// Manually get the physical address of a virtual address in this address space. \
    pub fn get_phys(&self, virt_addr: u64) -> Option<PhysAddr> {
        self.page_tables.translate(VirtAddr::new(virt_addr) )
    }

    /// Copy `total_bytes_to_copy` from `src_ptr` in the `self` address space to `dest_page_start` in the `dest_process` address space. \
    /// Destination addresses are manually retrieved from the page tables of the `dest_process`. \
    /// If `fill_up_with_zeroes` is true, the remaining bytes in the last page will be filled with zeroes.
    pub unsafe fn copy_to_addr_space(
        &self, src_ptr: *const u8, dest_space: &VirtualAddressSpace, dest_page_start: Page, total_bytes_to_copy: u64, fill_up_with_zeroes: bool,
    ) {
        // Calc number of pages to be copied
        let pages_to_copy = if total_bytes_to_copy as usize % PAGE_SIZE == 0 {
            total_bytes_to_copy as usize / PAGE_SIZE
        } else {
            (total_bytes_to_copy as usize / PAGE_SIZE) + 1
        };

        unsafe {
            let mut bytes_to_copy = 0;
            let mut offset = 0;

            let mut dest_phys_addr = dest_space.get_phys(dest_page_start.start_address().as_u64()).expect("get_phys failed");
            let mut dest = dest_phys_addr.as_u64() as *mut u8;
            for _i in 0..pages_to_copy {
                // get destination physical address
                dest_phys_addr = dest_space.get_phys(dest_page_start.start_address().as_u64() + offset).expect("get_phys failed");
                dest = dest_phys_addr.as_u64() as *mut u8;

                // source virtual address
                let source_addr = src_ptr.offset(offset as isize);

                // calc number of bytes to copy
                if total_bytes_to_copy - offset < PAGE_SIZE as u64 {
                    // if we are at the last page, copy only the remaining bytes
                    bytes_to_copy = total_bytes_to_copy - offset;
                } else {
                    bytes_to_copy = PAGE_SIZE as u64;
                }

                // copy code bytes
                dest.copy_from(source_addr, bytes_to_copy as usize);

                offset += bytes_to_copy;
            }

            // fill up last code page with zeroes if not fully used
            if fill_up_with_zeroes {
                let rest_bytes_to_copy = PAGE_SIZE as u64 - bytes_to_copy;
                if rest_bytes_to_copy > 0 {
                    dest.offset(offset as isize).write_bytes(0, rest_bytes_to_copy as usize);
                }
            }
        }
    }
}

impl Drop for VirtualAddressSpace {
    fn drop(&mut self) {
        for vma in self.virtual_memory_areas.read().iter() {
            self.page_tables.unmap(vma.range(), true);
        }
    }
}
