/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: paging                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Public functions related to paging, protection, and memory mapping.     ║
   ║   - map           map a range of pages to the given memory space        ║
   ║   - map_physical  map a range of frames to the given page range in the  ║
   ║                   in the given memory space                             ║
   ║   - set_flags     set flags of page table entries for a range of pages  ║
   ║   - translate     translate a virtual address to a physical address     ║
   ║   - unmap         unmap a range of pages                                ║
   ║   - page_from_u64 convert a u64 address to a Page                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 24.5.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use core::cmp::min;
use core::{fmt, ptr};
use log::{debug, info, warn};
use spin::RwLock;
use x86_64::instructions::tlb;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::Size4KiB;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::{Page, PageRange};
use x86_64::structures::paging::{PageTable, PageTableFlags, PageTableIndex, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};

use crate::memory::{self, MemorySpace, PAGE_SIZE};

/// Helper function to convert a u64 address to a PhysFrame.
pub fn page_from_u64(addr: u64) -> Result<Page<Size4KiB>, x86_64::structures::paging::page::AddressNotAligned> {
    Page::from_start_address(VirtAddr::new(addr))
}

/// Address space for a process
pub struct Paging {
    root_table: RwLock<*mut PageTable>, // Root page table (pml4)
    depth: usize,                       // Depth of the page table hierarchy
}

unsafe impl Send for Paging {}
unsafe impl Sync for Paging {}

pub fn page_table_index(virt_addr: VirtAddr, level: usize) -> PageTableIndex {
    PageTableIndex::new_truncate((virt_addr.as_u64() >> 12 >> ((level as u8 - 1) * 9)) as u16)
}

impl Drop for Paging {
    fn drop(&mut self) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        Paging::drop_table(root_table, depth);
    }
}

impl Paging {
    /// Create a new root page table for address space `self` with the given `depth`
    pub(super) fn new(depth: usize) -> Self {
        let table_addr = memory::alloc_frames(1).start;
        let root_table = table_addr.start_address().as_u64() as *mut PageTable;
        unsafe {
            root_table.as_mut().unwrap().zero();
        }

        Self {
            root_table: RwLock::new(root_table),
            depth,
        }
    }

    /// Create a new address space from `other` address space (copying all page tables)
    pub fn from_other(other: &Paging) -> Self {
        let address_space = Paging::new(other.depth);

        {
            let root_table_guard = address_space.root_table.write();
            let root_table = unsafe { root_table_guard.as_mut().unwrap() };
            let other_root_table_guard = other.root_table.read();
            let other_root_table = unsafe { other_root_table_guard.as_ref().unwrap() };

            Paging::copy_table(other_root_table, root_table, other.depth);
        }

        address_space
    }

    /// Load cr3 register with the root page table address of `self`
    pub(super) fn load(&self) {
        unsafe { Cr3::write(PhysFrame::from_start_address(self.page_table_address()).unwrap(), Cr3Flags::empty()) };
    }

    /// Return physical address of root page table address (pml4) of `self`
    pub(super) fn page_table_address(&self) -> PhysAddr {
        // Get root table pointer without locking.
        // We cannot use the lock here, because this function is called by the scheduler.
        // This is still safe, since we only return an address and not a reference.
        let root_table = unsafe { self.root_table.as_mut_ptr().read() };
        PhysAddr::new(root_table as u64)
    }

    /// Map page range `pages` to the given memory `space` with the given page table entry `flags` \
    /// If `space` is `MemorySpace::Kernel`, the frames are not allocated but pages are identity mapped. \
    /// If `space` is `MemorySpace::User` and if frames.start = frames.end: frames are allocated from the frame allocator.
    /// Otherwise the given `frames` are used for the mapping
    pub(super) fn map(&self, pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };
        let frames = PhysFrameRange {
            start: PhysFrame::from_start_address(PhysAddr::zero()).unwrap(),
            end: PhysFrame::from_start_address(PhysAddr::zero()).unwrap(),
        };
        Paging::map_in_table(root_table, frames, pages, space, flags, depth);
    }

    /// Map a range of `frames` to the given page range `pages` in the given memory `space` with the given page table entry `flags` \
    /// This is only allowed for `space`set to `MemorySpace::User`
    pub(super) fn map_physical(&self, frames: PhysFrameRange, pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        // Check if the number of frames matches the number of pages
        assert_eq!(frames.end - frames.start, pages.end - pages.start);
        Paging::map_in_table(root_table, frames, pages, space, flags, depth);
    }

    /// Return physical address for the give a virtual address `addr`
    pub(super) fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        let depth = self.depth;
        let root_table_guard = self.root_table.read();
        let root_table = unsafe { (*root_table_guard).as_ref().unwrap() };
        Paging::translate_in_table(root_table, addr, depth)
    }

    /// Unmap a range of `pages` from the address space.
    /// `free_physical` indicates if the physical frames should be freed.
    pub(super) fn unmap(&self, pages: PageRange, free_physical: bool) {
        let depth = self.depth;
        // Write lock: unmapping clears entries and frees frames and page tables
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        Paging::unmap_in_table(root_table, pages, depth, free_physical);
    }

    /// Set `flags` of page table entries for the give range of `pages``
    pub(super) fn set_flags(&self, pages: PageRange, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };

        Paging::set_flags_in_table(root_table, pages, flags, depth);
    }

    pub fn dump(&self) {
        // TODO: A read lock should be enough, maybe we can do without unsafe?
        let root_table_guard = self.root_table.write();
        let root_table = unsafe { root_table_guard.as_mut().unwrap() };
        let mut area: PageTableArea = PageTableArea::new(None, 0);

        debug!("Dumping page tables");

        Paging::dump_table(root_table, 0, 4, &mut area);
    }

    fn dump_table(table: &PageTable, base_address: usize, level: usize, area: &mut PageTableArea) {
        let mut entry_address = base_address;

        for (index, entry) in table.iter().enumerate() {
            entry_address = base_address + (index << (12 + (level - 1) * 9));

            if !entry.is_unused() {
                if level > 1 {
                    let next_level = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                    Paging::dump_table(next_level, entry_address, level - 1, area);
                } else {
                    area.check_and_set(PageTableAreaType::Offset(entry_address as u64 - entry.addr().as_u64()), entry_address);
                }
            } else {
                area.check_and_set(PageTableAreaType::Empty, entry_address);
            }
        }

        // If we are at the end of the top level page directory, we need to print the last detected area
        if level == 4 {
            let end_address = (entry_address + 1) << (12 + level * 9);
            area.check(end_address);
        }
    }

    /// Internal recursive function to copy page tables from `source` to `target`
    fn copy_table(source: &PageTable, target: &mut PageTable, level: usize) {
        if level > 1 {
            // On all levels larger than 1, we allocate new page frames
            for (index, target_entry) in target.iter_mut().enumerate() {
                let source_entry = &source[index];
                if source_entry.is_unused() {
                    // Skip empty entries
                    target_entry.set_unused();
                    continue;
                }

                let phys_frame = memory::alloc_frames(1).start;
                let flags = source[index].flags();
                target_entry.set_frame(phys_frame, flags);

                let next_level_source = unsafe { (source_entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                let next_level_target = unsafe { (target_entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                Paging::copy_table(next_level_source, next_level_target, level - 1);
            }
        } else {
            // Only on the last level, we create a 1:1 copy of the page table
            for (index, target_entry) in target.iter_mut().enumerate() {
                let source_entry = &source[index];
                target_entry.set_addr(source_entry.addr(), source_entry.flags());
            }
        }
    }

    /// Internal recursive function to map a range of `frames` to the given page range `pages` in the given memory `space` with the given page table entry `flags`. \
    /// If `space` is `MemorySpace::Kernel`, the frames are not allocated but pages are identity mapped. \
    /// If `space` is `MemorySpace::User` and if frames.start = frames.end: frames are allocated from the frame allocator.
    /// Otherwise the given `frames` are used for the mapping
    fn map_in_table(table: &mut PageTable, mut frames: PhysFrameRange, mut pages: PageRange, space: MemorySpace, flags: PageTableFlags, level: usize) -> usize {
        let mut total_allocated_pages: usize = 0;
        let start_index = usize::from(page_table_index(pages.start.start_address(), level));

        if level > 1 {
            // Calculate next level page table until level == 1
            for entry in table.iter_mut().skip(start_index) {
                let next_level_table;
                if entry.is_unused() {
                    // Entry is empty -> Allocate new page frame
                    let phys_frame = memory::alloc_frames(1).start;
                    entry.set_frame(phys_frame, flags);

                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                    next_level_table.zero();
                } else {
                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                }

                let allocated_pages = Paging::map_in_table(next_level_table, frames, pages, space, flags, level - 1);
                pages = PageRange {
                    start: pages.start + allocated_pages as u64,
                    end: pages.end,
                };
                total_allocated_pages += allocated_pages;

                if frames.end > frames.start {
                    frames = PhysFrameRange {
                        start: frames.start + allocated_pages as u64,
                        end: frames.end,
                    };
                }

                if pages.start >= pages.end {
                    break;
                }
            }
        } else {
            // Reached level 1 page table
            total_allocated_pages += match space {
                MemorySpace::Kernel => Paging::identity_map_kernel(table, pages, flags),
                MemorySpace::User => {
                    if frames.start == frames.end {
                        Paging::map_user(table, pages, flags)
                    } else {
                        Paging::map_user_physical(table, frames, pages, flags)
                    }
                }
            }
        }

        total_allocated_pages
    }

    /// Teil von `pages`, der in den Eintrag von `pages.start` auf Ebene `level` fällt
    fn part_in_entry(pages: PageRange, level: usize) -> PageRange {
        let per_entry = 1u64 << (9 * (level - 1)); // Seiten pro Eintrag
        let offset = (pages.start.start_address().as_u64() >> 12) & (per_entry - 1);
        let len = (per_entry - offset).min(pages.end - pages.start);
        PageRange {
            start: pages.start,
            end: pages.start + len,
        }
    }

    /// Internal recursive function to unmap a range of `pages` where `free_phyisical` defines if frame should be freed.
    fn unmap_in_table(table: &mut PageTable, pages: PageRange, level: usize, free_physical: bool) {
        let mut rest = pages;
        while rest.start < rest.end {
            let part = Self::part_in_entry(rest, level);
            let entry = &mut table[page_table_index(part.start.start_address(), level)];

            if !entry.is_unused() {
                if level > 1 {
                    let next = unsafe { &mut *(entry.addr().as_u64() as *mut PageTable) };
                    Paging::unmap_in_table(next, part, level - 1, free_physical);
                    if Paging::is_table_empty(next) {
                        let frame = PhysFrame::from_start_address(entry.addr()).unwrap();
                        entry.set_unused();
                        tlb::flush(part.start.start_address()); // flush potential TLB entry
                        memory::free_frames(PhysFrameRange { start: frame, end: frame + 1 });
                    }
                } else {
                    let frame = PhysFrame::from_start_address(entry.addr()).unwrap();
                    entry.set_unused();
                    tlb::flush(part.start.start_address()); // remove potential TLB entry
                    if free_physical {
                        memory::free_frames(PhysFrameRange { start: frame, end: frame + 1 });
                    }
                }
            }

            rest.start = part.end; // immer weiter, auch wenn der Eintrag leer war
        }
    }

    /// Internal recursive function to delete page tables
    fn drop_table(table: &mut PageTable, level: usize) {
        if level > 1 {
            // Calculate next level page table until level == 1
            for entry in table.iter_mut() {
                if entry.addr() == PhysAddr::zero() {
                    continue;
                }

                let next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                Paging::drop_table(next_level_table, level - 1);
            }
        }

        // Clear table
        table.iter_mut().for_each(|entry| entry.set_unused());

        let table_frame = PhysFrame::from_start_address(PhysAddr::new(ptr::from_ref(table) as u64)).unwrap();
        memory::free_frames(PhysFrameRange {
            start: table_frame,
            end: table_frame + 1,
        });
    }

    /// Internal recursive function to set `flags` in page table entries for a range of `pages`.
    fn set_flags_in_table(table: &mut PageTable, pages: PageRange, flags: PageTableFlags, level: usize) {
        let mut rest = pages;
        while rest.start < rest.end {
            let part = Paging::part_in_entry(rest, level);
            let entry = &mut table[page_table_index(part.start.start_address(), level)];

            if !entry.is_unused() {
                if level > 1 {
                    let next = unsafe { &mut *(entry.addr().as_u64() as *mut PageTable) };
                    Paging::set_flags_in_table(next, part, flags, level - 1);
                } else {
                    entry.set_flags(flags);                    
                    tlb::flush(part.start.start_address()); // flush potential TLB entry
                }
            }

            rest.start = part.end; // always move on, even if the entry was empty
        }
    }

    /// Internal recursive function returning physical address for the given virtual address `addr` or None.
    fn translate_in_table(table: &PageTable, addr: VirtAddr, level: usize) -> Option<PhysAddr> {
        let aligned_addr = addr.align_down(PAGE_SIZE as u64);
        let index = usize::from(page_table_index(aligned_addr, level));
        let entry = &table[index];

        if !entry.flags().contains(PageTableFlags::PRESENT) {
            return None;
        }

        if level > 1 {
            // Calculate next level page table until level == 1
            let next_level_table = unsafe { (entry.addr().as_u64() as *const PageTable).as_ref().unwrap() };
            Paging::translate_in_table(next_level_table, addr, level - 1)
        } else {
            // Reached level 1 page table
            Some(entry.addr() + (addr - aligned_addr))
        }
    }

    /// Create 1:1 mapping entries in the given page `table` for `pages` with the given `flags` for the kernel space.
    fn identity_map_kernel(table: &mut PageTable, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min((pages.end - pages.start) as usize, 512 - start_index);
        let mut frame_addr = PhysAddr::new(pages.start.start_address().as_u64());

        for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
            if count >= alloc_count {
                break;
            }

            // Overwriting is intended here: at boot the kernel maps all physical memory 1:1,
            // later parts of it are mapped again 
            let was_present = !entry.is_unused();
            entry.set_addr(frame_addr, flags);
            if was_present {
                tlb::flush((pages.start + count as u64).start_address()); // flush potential TLB entry
            }
            frame_addr += PAGE_SIZE as u64;
        }

        alloc_count
    }

    /// Create mapping entries in the given page `table` for `pages` with the given `flags` using freshly allocated physical frames.
    fn map_user(table: &mut PageTable, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min((pages.end - pages.start) as usize, 512 - start_index);

        for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
            if count >= alloc_count {
                break;
            }

            // Allocate new frame only if necessary
            if !entry.is_unused() {
                if entry.flags().contains(PageTableFlags::PRESENT) {
                    // Already mapped?
                    debug!("map_user: page {:?} is already mapped, keeping its frame", pages.start + count as u64);
                } else {
                    // Entry still holds a frame but is not present (flags without PRESENT):
                    // reuse its frame and make it present again, otherwise a page fault
                    // on this page would repeat forever.
                    // No TLB flush needed, the CPU does not cache non-present entries.
                    let frame_addr = entry.addr();
                    entry.set_addr(frame_addr, flags);
                }
                continue;
            }
            let phys_frame = memory::alloc_frames(1).start;
            //info!("map_user: page: {:?} phys_frame: {:?}", pages.start + count as u64, phys_frame);
            entry.set_frame(phys_frame, flags);
        }

        alloc_count
    }

    /// Create mapping entries in the given page `table` for `pages` using `frames` with the given `flags`.
    fn map_user_physical(table: &mut PageTable, frames: PhysFrameRange, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min((pages.end - pages.start) as usize, 512 - start_index);
        let mut frame_iter = frames.into_iter();

        for (count, entry) in table.iter_mut().skip(start_index).enumerate() {
            if count >= alloc_count {
                break;
            }

            let page = pages.start + count as u64;
            let frame = frame_iter.next().unwrap();
            let was_present = !entry.is_unused();
            if was_present && entry.addr() != frame.start_address() {
                // Remapping to a different frame: the old frame is no longer reachable from here
                warn!(
                    "map_user_physical: page {:?} was mapped to {:?}, remapping to {:?}",
                    page,
                    entry.addr(),
                    frame.start_address()
                );
            }
            entry.set_frame(frame, flags);
            if was_present {
                // The TLB may still hold the old mapping
                tlb::flush(page.start_address());
            }
        }

        alloc_count
    }

    /// Check if a page table is empty.
    fn is_table_empty(table: &PageTable) -> bool {
        for entry in table.iter() {
            if !entry.is_unused() {
                return false;
            }
        }

        true
    }
}

// Data structures for printing and dumping page tables

/// A struct for storing the address of a page table entry, allowing to pretty-print the position of this page table entry in the page table tree (e.g. 1.2.5.2 = 2nd entry in the fifth level-three page of the second level-two page of the first level-one page)
pub struct PageTableEntryAddress {
    address: usize,
    level: usize,
}

impl PageTableEntryAddress {
    pub fn new(address: usize, level: usize) -> Self {
        Self { address, level }
    }

    pub fn get_pml_part(&self, level: usize) -> usize {
        let shift_bit_count = 12 + ((level - 1) * 9);
        let mask = ((1 << 9) - 1) << shift_bit_count; // PML entries are 9 bit wide
        return (self.address & mask) >> shift_bit_count;
    }
}

impl fmt::Debug for PageTableEntryAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PageTableEntry ").unwrap();
        for level in (self.level..5).rev() {
            write!(f, "{}", self.get_pml_part(level)).unwrap();
            if level != 1 {
                write!(f, ".").unwrap();
            }
        }
        Ok(())
    }
}

/// Enum to store whether an area of continuous page table entries is either empty or a linear mapping with a given offset
#[derive(Debug, PartialEq)]
enum PageTableAreaType {
    Empty,
    Offset(u64),
}

/// Struct to store the type and the start of an area of continuous page table entries of the same type
struct PageTableArea {
    area_type: Option<PageTableAreaType>,
    start_address: usize,
}

/// Functions to actually dump the page table by printing the size and start and end addresses of continous page table areas whenever the area type changes (e.g. an identity mapping ends and the following page table entries are empty)
impl PageTableArea {
    pub fn new(area_type: Option<PageTableAreaType>, start_address: usize) -> Self {
        PageTableArea { area_type, start_address }
    }

    pub fn check(&mut self, current_address: usize) {
        if let Some(value) = &self.area_type {
            info!(
                "{:?} mapping for addresses 0x{:x} - 0x{:x}, {:?} - {:?}",
                value,
                self.start_address,
                current_address - 1,
                PageTableEntryAddress::new(self.start_address, 1),
                PageTableEntryAddress::new(current_address - 1, 1)
            );
            self.area_type = None
        }
    }

    pub fn check_and_set(&mut self, current_area_type: PageTableAreaType, current_address: usize) {
        if let Some(value) = &self.area_type {
            if *value != current_area_type {
                // We have a different area type now, so print the old one and store a new start address
                self.check(current_address);

                self.area_type = Some(current_area_type);
                self.start_address = current_address;
            }
        } else {
            self.area_type = Some(current_area_type);
            self.start_address = current_address;
        }
    }
}
