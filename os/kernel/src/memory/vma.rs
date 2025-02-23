/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: vma                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Virtual Memory Areas.                                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Univ. Duesseldorf, 20.2.2025                    ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::VirtAddr;
use x86_64::structures::paging::page::PageRange;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process_manager;

///
/// Description: Address space for a process
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct VirtualMemoryArea {
    range: PageRange,
    typ: VmaType
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum VmaType {
    Code, Heap, Stack, Environment
}

impl VirtualMemoryArea {
    pub const fn new(range: PageRange, typ: VmaType) -> Self {
        Self { range, typ }
    }

    pub fn from_address(start: VirtAddr, size: usize, typ: VmaType) -> Self {
        let start_page = Page::from_start_address(start).expect("VirtualMemoryArea: Address is not page aligned");
        let range = PageRange { start: start_page, end: start_page + (size / PAGE_SIZE) as u64 };

        Self { range, typ }
    }

    pub fn start(&self) -> VirtAddr {
        self.range.start.start_address()
    }

    pub fn end(&self) -> VirtAddr {
        self.range.end.start_address()
    }

    pub fn range(&self) -> PageRange {
        self.range
    }

    pub fn typ(&self) -> VmaType {
        self.typ
    }

    pub fn overlaps_with(&self, other: &VirtualMemoryArea) -> bool {
        if self.range.end <= other.range.start || self.range.start >= other.range.end {
            false
        } else {
            true
        }
    }

    pub fn grow_downwards(&self, pages: usize) {
        let new_pages = PageRange { start: self.range.start - pages as u64, end: self.range.start };
        let process = process_manager().read().current_process();

        process.address_space().map(new_pages, MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
        process.update_vma(*self, |vma| vma.range.start = new_pages.start);
    }
}

