/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: virtual memory area                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Functions related to a virtual memory area.                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland and Michael Schoettner                           ║
   ║         Univ. Duesseldorf, 18.07.2025                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use core::fmt;
use x86_64::VirtAddr;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PageTableFlags};
use crate::memory::{MemorySpace,PAGE_SIZE};


#[derive(Copy, Clone, PartialEq, Debug)]
pub enum VmaType {
    Code,
    Heap,
    Environment,
    DeviceMemory,
    UserStack,
    KernelStack,
    Anonymous,
}

pub const TAG_SIZE: usize = 8; // Define a constant for tag size in bytes

#[derive(Copy, Clone, PartialEq)]
pub struct VirtualMemoryArea {
    pub space: MemorySpace,
    pub range: PageRange,
    pub typ: VmaType,
    pub tag: [u8; TAG_SIZE], // 6-byte tag name (for debugging)
}

impl VirtualMemoryArea {
    /// Create a new VirtualMemoryArea with a given range and type and a tag name
    pub const fn new_with_tag(
        space: MemorySpace,
        range: PageRange,
        typ: VmaType,
        tag_str: &str,
    ) -> Self {
        let mut tag: [u8; TAG_SIZE] = [b'-'; TAG_SIZE];
        let tag_bytes = tag_str.as_bytes();
        let len = if tag_bytes.len() > TAG_SIZE {
            TAG_SIZE
        } else {
            tag_bytes.len()
        };

        if len > 0 {
            let mut i = 0;
            while i < len {
                tag[i] = tag_bytes[i];
                i += 1;
            }
        }
        Self {
            space,
            range,
            typ,
            tag,
        }
    }

    /// Alternatively, create a new VirtualMemoryArea using the thread id `tid` as tag
    pub const fn new_with_id(
        space: MemorySpace,
        range: PageRange,
        typ: VmaType,
        tid: usize,
    ) -> Self {
        let mut tag: [u8; TAG_SIZE] = [b'-'; TAG_SIZE]; // Default to dashes ('------')
        let mut num = tid;
        let mut i = TAG_SIZE;

        while num > 0 && i > 0 {
            i -= 1;
            tag[i] = b'0' + (num % 10) as u8; // Convert last digit to ASCII
            num /= 10;
        }

        Self {
            space,
            range,
            typ,
            tag,
        }
    }

    /// Create a new VirtualMemoryArea from a virtual `start` address and `size` with `typ`
    pub fn from_address(start: VirtAddr, size: usize, space: MemorySpace, typ: VmaType) -> Self {
        let start_page = Page::from_start_address(start)
            .expect("VirtualMemoryArea: Address is not page aligned");

        // Calculate the number of pages needed
        let mut count_pages = (size / PAGE_SIZE) as u64;
        if size % PAGE_SIZE != 0 {
            count_pages += 1;
        }

        // Init PageRange
        let range = PageRange {
            start: start_page,
            end: start_page + count_pages, // PageRange end is exclusive
        };

        let tag: [u8; TAG_SIZE] = [b'-'; TAG_SIZE];
        Self {
            space,
            range,
            typ,
            tag,
        }
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
        self.range.end > other.range.start && self.range.start < other.range.end
    }

    /// Helper function to check if flags are consistent with the vma
    pub fn check_and_enforce_consistency(&self, mut flags: PageTableFlags) -> PageTableFlags {
        match self.space {
            MemorySpace::User => {
                flags |= PageTableFlags::USER_ACCESSIBLE;
            }
            MemorySpace::Kernel => {
                flags &= !PageTableFlags::USER_ACCESSIBLE;
            }
        }
        flags
    }

    /// Helper function to check if two virtual address spaces are equivalent.
    pub fn is_equivalent_to(&self, other: &Self) -> bool {
        self.start() == other.start() &&
        self.end() == other.end() &&
        self.typ == other.typ &&
        self.space == other.space &&
        self.tag == other.tag
    }

}

impl fmt::Debug for VirtualMemoryArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Convert tag bytes to a readable string
        let tag_str = core::str::from_utf8(&self.tag).unwrap_or("<invalid>"); // Handle potential invalid UTF-8

        write!(
            f,
            "   VMA: Space: {:?}, Type: {:?}, [0x{:x}; 0x{:x}], #pages: {}, tag: {:?}",
            self.space,
            self.typ,
            self.range.start.start_address().as_u64(),
            self.range.end.start_address().as_u64(),
            (self.range.end.start_address().as_u64() - self.range.start.start_address().as_u64())
                / PAGE_SIZE as u64,
            tag_str
        )
    }
}
