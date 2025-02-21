/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: virtual                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Functions related to paging, protection, and memory mapping.    ║
   ║ Funcs:                                                                  ║
   ║   - map:          map a range of pages to the given memory space        ║
   ║   - unmap:        unmap a range of pages from the address space         ║
   ║   - map_physical: map a range of frames to the given page range in the  ║ 
   ║                   in the given memory space                             ║
   ║   - translate:    translate a virtual address to a physical address     ║
   ║   - set_flags:    set flags of page table entries for a range of pages  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 20.2.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::cmp::min;
use core::ptr;
use spin::RwLock;
use x86_64::structures::paging::{PageTable, PageTableFlags, PageTableIndex, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;

use crate::memory::{MemorySpace, PAGE_SIZE, physical};

///
/// Description: Address space for a process
///
pub struct AddressSpace {
    root_table: RwLock<*mut PageTable>, // Root page table (pml4)
    depth: usize  // Depth of the page table hierarchy
}

unsafe impl Send for AddressSpace {}
unsafe impl Sync for AddressSpace {}

pub fn page_table_index(virt_addr: VirtAddr, level: usize) -> PageTableIndex {
    PageTableIndex::new_truncate((virt_addr.as_u64() >> 12 >> ((level as u8 - 1) * 9)) as u16)
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        AddressSpace::drop_table(root_table, depth);
    }
}

impl AddressSpace {

    ///
    /// Description: Create a new root page table for this address space
    ///
    pub fn new(depth: usize) -> Self {
        let table_addr = physical::alloc(1).start;
        let root_table = table_addr.start_address().as_u64() as *mut PageTable;
        unsafe { root_table.as_mut().unwrap().zero(); }

        Self { root_table: RwLock::new(root_table), depth }
    }

    ///
    /// Description: Create a new address space from another address space (copying all page tables)
    ///
    /// Parameters: `other` address space to be copied 
    /// 
    pub fn from_other(other: &AddressSpace) -> Self {
        let address_space = AddressSpace::new(other.depth);

        {
            let root_table_guard = address_space.root_table.write();
            let root_table = unsafe { root_table_guard.as_mut().unwrap() };
            let other_root_table_guard = other.root_table.read();
            let other_root_table = unsafe { other_root_table_guard.as_ref().unwrap() };

            AddressSpace::copy_table(other_root_table, root_table, other.depth);
        }

        address_space
    }

    ///
    /// Description: Load cr3 register with the root page table address
    ///
    pub fn load(&self) {
        unsafe { Cr3::write(PhysFrame::from_start_address(self.page_table_address()).unwrap(), Cr3Flags::empty()) };
    }


    ///
    /// Description: Get root page table address (pml4) 
    ///
    pub fn page_table_address(&self) -> PhysAddr {
        // Get root table pointer without locking.
        // We cannot use the lock here, because this function is called by the scheduler.
        // This is still safe, since we only return an address and not a reference.
        let root_table = unsafe { self.root_table.as_mut_ptr().read() };
        PhysAddr::new(root_table as u64)
    }

    ///
    /// Description: Map a range of pages to the given memory space
    ///
    /// Parameters: \
    ///  `pages` page range to map \
    ///  `space` kernel or user space \
    ///  `flags` page table entry flags
    /// 
    pub fn map(&self, pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };
        let frames = PhysFrameRange { start: PhysFrame::from_start_address(PhysAddr::zero()).unwrap(), end: PhysFrame::from_start_address(PhysAddr::zero()).unwrap() };

        AddressSpace::map_in_table(root_table, frames, pages, space, flags, depth);
    }

    ///
    /// Description: Map a range of frames to the given page range in the given memory space
    ///
    /// Parameters: \
    ///  `frames` frame range to map \
    ///  `pages`  page range to be used for the mapping \
    ///  `space` kernel or user space \
    ///  `flags` page table entry flags
    /// 
    pub fn map_physical(&self, frames: PhysFrameRange, pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        // Check if the number of frames matches the number of pages
        assert_eq!(frames.end - frames.start, pages.end - pages.start);
        AddressSpace::map_in_table(root_table, frames, pages, space, flags, depth);
    }

    ///
    /// Description: Translate a virtual address to a physical address
    ///
    /// Parameters: `addr` virtual address to be translated
    /// 
    /// Return: physical address
    /// 
    pub fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        let depth = self.depth;
        let root_table_guard = self.root_table.read();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        AddressSpace::translate_in_table(root_table, addr, depth)
    }

    ///
    /// Description: Unmap a range of pages from the address space
    ///
    /// Parameters: \
    ///   `pages`         page range \
    ///   `free_physical` flag to indicate if the physical frames should be freed
    /// 
    pub fn unmap(&self, pages: PageRange, free_physical: bool) {
        let depth = self.depth;
        let root_table_guard = self.root_table.read();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        AddressSpace::unmap_in_table(root_table, pages, depth, free_physical);
    }

    ///
    /// Description: Set flags of page table entries for a range of pages 
    ///
    /// Parameters: \
    ///   `pages`  page range \
    ///   `flags`  page table entry flags
    ///   
    pub fn set_flags(&self, pages: PageRange, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        AddressSpace::set_flags_in_table(root_table, pages, flags, depth);
    }

    ///
    /// Description: Internal recursive function to copy page tables
    ///
    /// Parameters: \
    ///  `source`  current source table \
    ///  `target`  current target table
    //
    fn copy_table(source: &PageTable, target: &mut PageTable, level: usize) {
        if level > 1 { // On all levels larger than 1, we allocate new page frames
            for (index, target_entry) in target.iter_mut().enumerate() {
                let source_entry = &source[index];
                if source_entry.is_unused() { // Skip empty entries
                    target_entry.set_unused();
                    continue;
                }

                let phys_frame = physical::alloc(1).start;
                let flags = source[index].flags();
                target_entry.set_frame(phys_frame, flags);

                let next_level_source = unsafe { (source_entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                let next_level_target = unsafe { (target_entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                AddressSpace::copy_table(next_level_source, next_level_target, level - 1);
            }
        } else { // Only on the last level, we create a 1:1 copy of the page table
            for (index, target_entry) in target.iter_mut().enumerate() {
                let source_entry = &source[index];
                target_entry.set_addr(source_entry.addr(), source_entry.flags());
            }
        }
    }

    ///
    /// Description: Internal recursive function to map a range of pages into page tables
    ///
    /// Parameters: \
    ///  `table`  current table \
    ///  `frames` frame range \
    ///  `space`  kernel or user space \
    ///  `flags`  page table entry flags \
    ///  `level`  current level of the page table hierarchy
    /// 
    fn map_in_table(table: &mut PageTable, mut frames: PhysFrameRange, mut pages: PageRange, space: MemorySpace, flags: PageTableFlags, level: usize) -> usize {
        let mut total_allocated_pages: usize = 0;
        let start_index = usize::from(page_table_index(pages.start.start_address(), level));

        if level > 1 { // Calculate next level page table until level == 1
            for entry in table.iter_mut().skip(start_index) {
                let next_level_table;
                if entry.is_unused() { // Entry is empty -> Allocate new page frame
                    let phys_frame = physical::alloc(1).start;
                    entry.set_frame(phys_frame, flags);

                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                    next_level_table.zero();
                } else {
                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                }

                let allocated_pages = AddressSpace::map_in_table(next_level_table, frames, pages, space, flags, level - 1);
                pages = PageRange { start: pages.start + allocated_pages as u64, end: pages.end };
                total_allocated_pages += allocated_pages;

                if frames.end > frames.start {
                    frames = PhysFrameRange { start: frames.start + allocated_pages as u64, end: frames.end };
                }

                if pages.start >= pages.end {
                    break;
                }
            }
        } else { // Reached level 1 page table
            total_allocated_pages += match space {
                MemorySpace::Kernel => AddressSpace::identity_map_kernel(table, pages, flags),
                MemorySpace::User => {
                    if frames.start == frames.end {
                        AddressSpace::map_user(table, pages, flags)
                    } else {
                        AddressSpace::map_user_physical(table, frames, pages, flags)
                    }
                }
            }
        }

        total_allocated_pages
    }

    ///
    /// Description: Internal recursive function to unmap a range of pages and free frames
    ///
    /// Parameters: \
    ///  `table`          current table \
    ///  `pages`          page range \
    ///  `level`          current level of the page table hierarchy \
    ///   `free_physical` flag to indicate if the physical frames should be freed
    /// 
    fn unmap_in_table(table: &mut PageTable, mut pages: PageRange, level: usize, free_physical: bool) -> usize {
        let mut total_freed_pages: usize = 0;
        let start_index = usize::from(page_table_index(pages.start.start_address(), level));

        if level > 1 { // Calculate next level page table until level == 1
            for entry in table.iter_mut().skip(start_index) {
                if entry.is_unused() {
                    continue;
                }

                let next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                let freed_pages = AddressSpace::unmap_in_table(next_level_table, pages, level - 1, free_physical);
                pages = PageRange { start: pages.start + freed_pages as u64, end: pages.end };
                total_freed_pages += freed_pages;

                if AddressSpace::is_table_empty(next_level_table) {
                    let table_frame = PhysFrame::from_start_address(entry.addr()).unwrap();
                    unsafe { physical::free(PhysFrameRange { start: table_frame, end: table_frame + 1 }); }
                    entry.set_unused();
                }

                if pages.start >= pages.end {
                    break;
                }
            }
        } else { // Reached level 1 page table
            let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
            let free_count = min((pages.end - pages.start) as usize, 512 - start_index);

            for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
                if count >= free_count {
                    break;
                }

                if !entry.is_unused() {
                    if free_physical {
                        let frame = PhysFrame::from_start_address(entry.addr()).unwrap();
                        unsafe { physical::free(PhysFrameRange { start: frame, end: frame + 1 }); }
                    }

                    entry.set_unused();
                }
            }

            return free_count;
        }

        total_freed_pages
    }

    ///
    /// Description: Internal recursive function to delete page tables
    ///
    /// Parameters: \
    ///  `table`          current table \
    ///  `level`          current level of the page table hierarchy \
    /// 
    fn drop_table(table: &mut PageTable, level: usize) {
        if level > 1 { // Calculate next level page table until level == 1
            for entry in table.iter_mut() {
                if entry.addr() == PhysAddr::zero() {
                    continue;
                }

                let next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                AddressSpace::drop_table(next_level_table, level - 1);
            }
        }

        // Clear table
        table.iter_mut().for_each(|entry| entry.set_unused());

        let table_frame = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(table) as u64)).unwrap();
        unsafe { physical::free(PhysFrameRange { start: table_frame, end: table_frame + 1 }); }
    }

    ///
    /// Description: Internal recursive function to set flags in page table entries for a range of pages
    ///
    /// Parameters: \
    ///  `table`  actual table \
    ///  `pages`  page range \
    ///  `flags`  page table entry flags \
    ///  `level`  current level of the page table hierarchy
    /// 
    fn set_flags_in_table(table: &mut PageTable, mut pages: PageRange, flags: PageTableFlags, level: usize) -> usize {
        let mut total_edited_pages: usize = 0;
        let start_index = usize::from(page_table_index(pages.start.start_address(), level));

        if level > 1 { // Calculate next level page table until level == 1
            for entry in table.iter_mut().skip(start_index) {
                if entry.is_unused() { // Skip empty entries
                    continue;
                }

                let next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };

                let edited_pages = AddressSpace::set_flags_in_table(next_level_table, pages, flags, level - 1);
                pages = PageRange { start: pages.start + edited_pages as u64, end: pages.end };
                total_edited_pages += edited_pages;

                if pages.start >= pages.end {
                    break;
                }
            }
        } else { // Reached level 1 page table
            let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
            let edit_count = min((pages.end - pages.start) as usize, 512 - start_index);

            for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
                if count >= edit_count {
                    break;
                }

                entry.set_flags(flags);
            }

            return edit_count;
        }

        total_edited_pages
    }

    ///
    /// Description: Internal recursive function to translate a virtual address to a physical address
    ///
    /// Parameters: \
    ///  `table`  current table \
    ///  `addr`   virtual address \
    ///  `level`  current level of the page table hierarchy
    /// 
    /// Return: PhysAddr or None
    /// 
    fn translate_in_table(table: &mut PageTable, addr: VirtAddr, level: usize) -> Option<PhysAddr> {
        let aligned_addr = addr.align_down(PAGE_SIZE as u64);
        let index = usize::from(page_table_index(aligned_addr, level));
        let entry = &table[index];
        if entry.is_unused() {
            return None;
        }

        if level > 1 { // Calculate next level page table until level == 1
            let next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
            AddressSpace::translate_in_table(next_level_table, addr, level - 1)
        } else { // Reached level 1 page table
            Some(entry.addr() + (addr - aligned_addr))
        }
    }

    ///
    /// Description: Create identity mapping for kernel space
    /// 
    /// Parameters: \
    ///  `table`  table to be used for the mapping \
    ///  `pages`  user page range to be mapped \
    ///  `flags`  page table entry flags
    /// 
    fn identity_map_kernel(table: &mut PageTable, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min((pages.end - pages.start) as usize, 512 - start_index);
        let mut frame_addr = PhysAddr::new(pages.start.start_address().as_u64());

        for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
            if count >= alloc_count {
                break;
            }

            entry.set_addr(frame_addr, flags);
            frame_addr = frame_addr + PAGE_SIZE as u64;
        }

        alloc_count
    }

    ///
    /// Description: Map a page range to the user space using newly allocated physical frames
    /// 
    /// Parameters: \
    ///  `pages`  user page range to be mapped \
    ///  `flags`  page table entry flags
    /// 
    fn map_user(table: &mut PageTable, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min((pages.end - pages.start) as usize, 512 - start_index);

        for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
            if count >= alloc_count {
                break;
            }

            let phys_frame = physical::alloc(1).start;
            entry.set_frame(phys_frame, flags);
        }

        alloc_count
    }

    ///
    /// Description: Map a range of physical frames to user space at given user page range
    /// 
    /// Parameters: \
    ///  `frames` page frame range to be mapped \
    ///  `pages`  user page range for the mapping \
    ///  `flags`  page table entry flags
    /// 
    fn map_user_physical(table: &mut PageTable, frames: PhysFrameRange, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min((pages.end - pages.start) as usize, 512 - start_index);
        let mut frame_iter = frames.into_iter().skip(start_index);

        for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
            if count >= alloc_count {
                break;
            }

            entry.set_frame(frame_iter.next().unwrap(), flags);
        }

        alloc_count
    }

    ///
    /// Description: Check if a page table is empty
    /// 
    /// Return: `true` if all entries are unused otherwise `false`
    /// 
    fn is_table_empty(table: &PageTable) -> bool {
        for entry in table.iter() {
            if !entry.is_unused() {
                return false;
            }
        }

        true
    }
}