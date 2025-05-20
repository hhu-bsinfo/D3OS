/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: virtual memory management                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Functions related to a virtual memory management of a process address   ║
   ║ space. This includes managing virtual memory areas as well as enforcing ║
   ║ mappings and access protection through paging.                          ║
   ║                                                                         ║
   ║ VirtualAddressSpace                                                     ║
   ║   - new                                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland and Michael Schoettner                           ║
   ║         Univ. Duesseldorf, 02.03.2025                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt;
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
use crate::memory::pages::Paging;
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::process_manager;

/*static LAST_VIRT_ADDR: Once<Mutex<Cell<VirtAddr>>> = Once::new();

pub fn intit() {
    LAST_VIRT_ADDR.call_once(|| Mutex::new(Cell::new(VirtAddr::new(0).unwrap())));
}
*/

/// Helper function to get the kernel page range from the given physical frame range (identity mapped)
pub fn kernel_page_range(frames: PhysFrameRange) -> PageRange {
    let kernel_pages = PageRange {
        start: Page::from_start_address(VirtAddr::new(frames.start.start_address().as_u64()))
            .unwrap(),
        end: Page::from_start_address(VirtAddr::new(frames.end.start_address().as_u64())).unwrap(),
    };
    kernel_pages
}

/// Clone address space. Used during process creation.
pub fn clone_address_space(other: &VirtualAddressSpace) -> Arc<Paging> {
    Arc::new(Paging::from_other(&&other.page_tables()))
}

/// Create kernel address space. Used during process creation.
pub fn create_kernel_address_space() -> Arc<Paging> {
    let address_space = Paging::new(4);
    let max_phys_addr = phys_limit().start_address();
    let range = PageRange {
        start: Page::containing_address(VirtAddr::zero()),
        end: Page::containing_address(VirtAddr::new(max_phys_addr.as_u64())),
    };

    address_space.map(
        range,
        MemorySpace::Kernel,
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
    );
    Arc::new(address_space)
}

/// All data related to a virtual address space of a process.
pub struct VirtualAddressSpace {
    virtual_memory_areas: RwLock<Vec<VirtualMemoryArea>>,
    page_tables: Arc<Paging>,
    first_usable_user_addr: VirtAddr,
    last_usable_user_addr: VirtAddr,
}

impl VirtualAddressSpace {
    pub fn new(page_tables: Arc<Paging>) -> Self {
        let first_usable_user_addr = VirtAddr::new(crate::consts::USER_SPACE_START as u64);
        let last_usable_user_addr: VirtAddr = VirtAddr::new(Self::last_usable_virtual_address());
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

    /// Return the last useable virtual address in canonical form
    fn last_usable_virtual_address() -> u64 {
        let virtual_bits = cpu().linear_address_bits();
        (1u64 << (virtual_bits - 1)) - 1
    }

    pub fn load_address_space(&self) {
        self.page_tables.load();
    }

    pub fn page_tables(&self) -> Arc<Paging> {
        Arc::clone(&self.page_tables)
    }

    /// Add a [`VirtualMemoryArea`] to this address space.
    ///
    /// This doesn't actually map it in, this only happens on memory access.
    pub fn add_vma(&self, new_area: VirtualMemoryArea) {
        // TODO: return an error instead of panicking
        let mut areas = self.virtual_memory_areas.write();
        match areas.iter().find(|area| area.overlaps_with(&new_area)) {
            Some(_) => panic!("Process: Trying to add a VMA, which overlaps with an existing one!"),
            None => areas.push(new_area),
        }
    }

    /// Return all vmas with the given type `typ` in his address space.
    pub fn find_vmas(&self, typ: VmaType) -> Vec<VirtualMemoryArea> {
        let mut found = Vec::<VirtualMemoryArea>::new();
        let areas = self.virtual_memory_areas.read();
        for area in areas.iter() {
            if area.typ() == typ {
                found.push(*area);
            }
        }

        // MS WARUM?
        found.sort_by(|first, second| {
            return if first.start().as_u64() < second.start().as_u64() {
                Ordering::Less
            } else if first.start().as_u64() > second.start().as_u64() {
                Ordering::Greater
            } else {
                Ordering::Equal
            };
        });

        found
    }

    /// Update the vma `vma` in this address space with the given `update` function.
    fn update_vma(&self, vma: VirtualMemoryArea, update: impl Fn(&mut VirtualMemoryArea)) {
        let mut areas = self.virtual_memory_areas.write();
        match areas.iter_mut().find(|area| **area == vma) {
            Some(area) => update(area),
            None => panic!("Trying to update a non-existent VMA!"),
        }
    }

    /// Map a [`VirtualMemoryArea`] to this address space.
    ///
    /// This randomly allocates and maps some available frames.
    pub fn map(&self, vma: VirtualMemoryArea, space: MemorySpace, flags: PageTableFlags) {
        let areas = self.virtual_memory_areas.read();
        areas
            .iter()
            .find(|area| **area == vma)
            .expect("tried to map a non-existent VMA!");
        self.page_tables.map(vma.range, space, flags);
    }

    /// Create a mapping for the given page range `pages` in this address space with the given `flags`
    /// and allocate frames for the `pages_to_alloc` (sub-)range.\
    /// This function is only for `MemorySpace::User`.
    pub fn map_and_allocate(
        &self,
        pages: PageRange,
        pages_to_alloc: PageRange,
        space: MemorySpace,
        flags: PageTableFlags,
        mem_type: VmaType,
        tag_str: &str,
    ) -> PhysFrameRange {
        assert!(
            space == MemorySpace::User,
            "map_and_allocate: space must be MemorySpace::User"
        );
        assert!(
            pages_to_alloc.start >= pages.start && pages_to_alloc.start < pages.end,
            "map_and_allocate: pages_to_alloc.start invalid"
        );
        assert!(
            pages_to_alloc.end > pages_to_alloc.start && pages_to_alloc.end <= pages.end,
            "map_and_allocate: pages_to_alloc.end invalid"
        );

        let page_count = pages_to_alloc.len();
        let frames = frames::alloc(page_count as usize);

      /*  info!(
            "map_and_allocate: page_count: {:?}, pages: {:?}, frames: {:?}",
            page_count, pages, frames
        );
*/
        let vma = VirtualMemoryArea::new_with_tag(pages, mem_type, tag_str);
        self.add_vma(vma);
        // TODO: this allocates the whole VMA and not just pages_to_alloc
        self.map_physical(vma, frames, space, flags);
        frames
    }

    /// Map a single page to this address space.
    pub fn map_single(
        &self,
        vma: VirtualMemoryArea,
        page: Page,
        space: MemorySpace,
        flags: PageTableFlags,
    ) {
        let areas = self.virtual_memory_areas.read();
        areas
            .iter()
            .find(|area| **area == vma)
            .expect("tried to map a non-existent VMA!");
        assert!(page.start_address() >= vma.start());
        assert!(page.start_address() + page.size() < vma.end());
        self.page_tables.map(
            PageRange {
                start: page,
                end: page + 1,
            },
            space,
            flags,
        );
    }

    /// Map the given physical frames `frames` to the virtual memory area `pages` in this address space
    pub fn map_physical(
        &self,
        vma: VirtualMemoryArea,
        frames: PhysFrameRange,
        space: MemorySpace,
        flags: PageTableFlags,
    ) {
        let page_count = frames.len();
    /*   info!(
            "map_physical: vma: {:?}, frames: {:?}, page_count: {:?}",
            vma, frames, page_count
        );*/
        let areas = self.virtual_memory_areas.read();
        areas
            .iter()
            .find(|area| **area == vma)
            .expect("tried to map a non-existent VMA!");
        self.page_tables
            .map_physical(frames, vma.range, space, flags);
    }

    /// Map the given physical frames `frames` to any virtual memory area in this address space
    pub fn map_io(&self, _frames: PhysFrameRange) {
        // self.add_vma(VirtualMemoryArea::new(pages, mem_type));
        // self.page_tables.map_physical(frames, pages, space, flags);
    }

    /// Map kernel stack of a thread
    pub fn map_kernel_stack(&self, pages: PageRange, tag_str: &str) {
        self.add_vma(VirtualMemoryArea::new_with_tag(
            pages,
            VmaType::KernelStack,
            tag_str,
        ));
        // no need for mapping in page tables because all frames are already identity mapped
    }

    pub fn set_flags(&self, pages: PageRange, flags: PageTableFlags) {
        self.page_tables.set_flags(pages, flags);
    }

    pub fn page_table_address(&self) -> PhysAddr {
        self.page_tables.page_table_address()
    }

    /// Dump all virtual memory areas of this address space
    pub fn dump(&self, pid: usize) {
        info!("VMAs of process [{}]", pid);
        let areas = self.virtual_memory_areas.read();
        for area in areas.iter() {
            info!("{:?}", area);
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

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum VmaType {
    Code,
    Heap,
    Environment,
    DeviceMemory,
    UserStack,
    KernelStack,
}

pub const TAG_SIZE: usize = 8; // Define a constant for tag size in bytes

#[derive(Copy, Clone, PartialEq)]
pub struct VirtualMemoryArea {
    pub range: PageRange,
    pub typ: VmaType,
    pub tag: [u8; TAG_SIZE], // 6-byte tag name (for debugging)
}

impl VirtualMemoryArea {
    /// Create a new VirtualMemoryArea with a given range and type and a tag name
    pub const fn new_with_tag(range: PageRange, typ: VmaType, tag_str: &str) -> Self {
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
        Self { range, typ, tag }
    }

    /// Alternatively, create a new VirtualMemoryArea using the thread id `tid` as tag
    pub const fn new_with_id(range: PageRange, typ: VmaType, tid: usize) -> Self {
        let mut tag: [u8; TAG_SIZE] = [b'-'; TAG_SIZE]; // Default to dashes ('------')
        let mut num = tid;
        let mut i = TAG_SIZE;

        while num > 0 && i > 0 {
            i -= 1;
            tag[i] = b'0' + (num % 10) as u8; // Convert last digit to ASCII
            num /= 10;
        }

        Self { range, typ, tag }
    }

    /// Create a new VirtualMemoryArea from a virtual `start` address and `size` with `typ`
    pub fn from_address(start: VirtAddr, size: usize, typ: VmaType) -> Self {
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
        Self { range, typ, tag }
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

    pub fn grow_downwards(&self, pages: usize) {
        let new_pages = PageRange {
            start: self.range.start - pages as u64,
            end: self.range.start,
        };
        let process = process_manager().read().current_process();

        process.virtual_address_space.page_tables().map(
            new_pages,
            MemorySpace::User,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
        process
            .virtual_address_space
            .update_vma(*self, |vma| vma.range.start = new_pages.start);
    }
}

impl fmt::Debug for VirtualMemoryArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Convert tag bytes to a readable string
        let tag_str = core::str::from_utf8(&self.tag).unwrap_or("<invalid>"); // Handle potential invalid UTF-8

        write!(
            f,
            "   VMA {:?}, [0x{:x}; 0x{:x}], #pages: {}, tag: {:?}",
            self.typ,
            self.range.start.start_address().as_u64(),
            self.range.end.start_address().as_u64(),
            (self.range.end.start_address().as_u64() - self.range.start.start_address().as_u64())
                / PAGE_SIZE as u64,
            tag_str
        )
    }
}
