use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::min;
use core::ops::Deref;
use spin::RwLock;
use x86_64::structures::paging::{Page, PageTable, PageTableFlags, PageTableIndex, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use crate::memory::{MemorySpace, PAGE_SIZE, physical};
use crate::memory::physical::phys_limit;

static ADDRESS_SPACES: RwLock<Vec<Arc<RwLock<AddressSpace>>>> = RwLock::new(Vec::new());

pub struct AddressSpace {
    root_table: *mut PageTable,
    depth: usize
}

unsafe impl Send for AddressSpace {}
unsafe impl Sync for AddressSpace {}

pub fn create_address_space() -> Arc<RwLock<AddressSpace>> {
    let mut address_spaces = ADDRESS_SPACES.write();

    if address_spaces.is_empty() {
        // Create kernel address space
        let address_space = Arc::new(RwLock::new(AddressSpace::new(4)));
        let max_phys_addr = phys_limit().start_address();
        let range = PageRange { start: Page::containing_address(VirtAddr::zero()), end: Page::containing_address(VirtAddr::new(max_phys_addr.as_u64())) };

        address_space.write().map(range, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        address_spaces.push(Arc::clone(&address_space));

        return Arc::clone(&address_space);
    } else {
        // Create user address space
        let kernel_space = address_spaces[0].read();
        let address_space = Arc::new(RwLock::new(AddressSpace::from_other(kernel_space.deref())));

        return Arc::clone(&address_space);
    }

}

pub fn current_address_space() -> Arc<RwLock<AddressSpace>> {
    let cr3 = Cr3::read();
    ADDRESS_SPACES.read().iter()
        .find(|address_space| address_space.read().root_table.cast_const() as u64 == cr3.0.start_address().as_u64())
        .unwrap()
        .clone()
}

pub fn kernel_address_space() -> Arc<RwLock<AddressSpace>> {
    ADDRESS_SPACES.read().get(0).expect("Trying to access kernel address space before initialization!").clone()
}

fn page_table_index(virt_addr: VirtAddr, level: usize) -> PageTableIndex {
    return PageTableIndex::new_truncate((virt_addr.as_u64() >> 12 >> ((level as u8 - 1) * 9)) as u16);
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        todo!()
    }
}

impl AddressSpace {
    pub fn new(depth: usize) -> Self {
        let table_addr = physical::alloc(1, MemorySpace::Kernel).start;
        let root_table = table_addr.start_address().as_u64() as *mut PageTable;
        unsafe { root_table.as_mut().unwrap().zero(); }

        Self { root_table, depth }
    }

    pub fn from_other(other: &AddressSpace) -> Self {
        let mut address_space = AddressSpace::new(other.depth);
        AddressSpace::copy_table(other.root_table(), address_space.root_table_mut(), other.depth);

        return address_space;
    }

    pub fn page_table_address(&self) -> PhysFrame {
        PhysFrame::from_start_address(PhysAddr::new(self.root_table.cast_const() as u64)).unwrap()
    }

    pub fn map(&mut self, pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table = self.root_table_mut();
        let frames = PhysFrameRange { start: PhysFrame::from_start_address(PhysAddr::zero()).unwrap(), end: PhysFrame::from_start_address(PhysAddr::zero()).unwrap() };

        AddressSpace::map_in_table(root_table, frames, pages, space, flags, depth);
    }

    pub fn map_physical(&mut self, frames: PhysFrameRange, pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
        let depth = self.depth;
        let root_table = self.root_table_mut();

        assert_eq!(frames.count(), pages.count());
        AddressSpace::map_in_table(root_table, frames, pages, space, flags, depth);
    }

    fn root_table(&self) -> &PageTable {
        unsafe { self.root_table.as_ref().unwrap() }
    }

    fn root_table_mut(&mut self) -> &mut PageTable {
        unsafe { self.root_table.as_mut().unwrap() }
    }

    fn copy_table(source: &PageTable, target: &mut PageTable, level: usize) {
        if level > 1 { // On all levels larger than 1, we allocate new page frames
            for (index, target_entry) in target.iter_mut().enumerate() {
                let source_entry = &source[index];
                if source_entry.is_unused() { // Skip empty entries
                    continue;
                }

                let phys_frame = physical::alloc(1, MemorySpace::Kernel).start;
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

    fn map_in_table(table: &mut PageTable, mut frames: PhysFrameRange, mut pages: PageRange, space: MemorySpace, flags: PageTableFlags, level: usize) -> usize {
        let mut total_allocated_pages: usize = 0;
        let start_index = usize::from(page_table_index(pages.start.start_address(), level));

        if level > 1 { // Calculate next level page table until level == 1
            for entry in table.iter_mut().skip(start_index) {
                let next_level_table;
                if entry.addr().is_null() { // Entry is empty -> Allocate new page frame
                    let phys_frame = physical::alloc(1, MemorySpace::Kernel).start;
                    entry.set_frame(phys_frame, flags);

                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                    next_level_table.zero();
                } else {
                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                }

                let allocated_pages = AddressSpace::map_in_table(next_level_table, frames, pages, space, flags, level - 1);
                pages = PageRange { start: pages.start + allocated_pages as u64, end: pages.end };
                total_allocated_pages = total_allocated_pages + allocated_pages;

                if frames.count() > 0 {
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
                    if frames.count() == 0 {
                        AddressSpace::map_user(table, pages, flags)
                    } else {
                        AddressSpace::map_user_physical(table, frames, pages, flags)
                    }
                }
            }
        }

        return total_allocated_pages;
    }

    fn identity_map_kernel(table: &mut PageTable, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min(pages.count(), 512 - start_index);
        let mut frame_addr = PhysAddr::new(pages.start.start_address().as_u64());

        for (index, entry) in table.iter_mut().skip(start_index).enumerate() {
            if index >= start_index + alloc_count {
                break;
            }

            entry.set_addr(frame_addr, flags);
            frame_addr = frame_addr + PAGE_SIZE;
        }

        return alloc_count;
    }

    fn map_user(table: &mut PageTable, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min(pages.count(), 512 - start_index);

        for (index, entry) in table.iter_mut().skip(start_index).enumerate() {
            if index >= start_index + alloc_count {
                break;
            }

            let phys_frame = physical::alloc(1, MemorySpace::User).start;
            entry.set_frame(phys_frame, flags);
        }

        return alloc_count;
    }

    fn map_user_physical(table: &mut PageTable, frames: PhysFrameRange, pages: PageRange, flags: PageTableFlags) -> usize {
        let start_index = usize::from(page_table_index(pages.start.start_address(), 1));
        let alloc_count = min(pages.count(), 512 - start_index);
        let mut frame_iter = frames.into_iter().skip(start_index);

        for (index, entry) in table.iter_mut().skip(start_index).enumerate() {
            if index >= start_index + alloc_count {
                break;
            }

            entry.set_frame(frame_iter.next().unwrap(), flags);
        }

        return alloc_count;
    }
}