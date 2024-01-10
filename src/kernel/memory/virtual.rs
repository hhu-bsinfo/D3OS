use core::cmp::min;
use log::info;
use spin::{Mutex, Once};
use x86_64::structures::paging::{Page, PageTable, PageTableFlags, PageTableIndex, PhysFrame};
use x86_64::{PhysAddr, VirtAddr};
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::page::PageRange;
use crate::kernel::memory::{MemorySpace, PAGE_SIZE, physical};
use crate::kernel::memory::physical::max_physical_address;

static VIRTUAL_MEMORY_MANAGER: Once<Mutex<VirtualMemoryManager>> = Once::new();

struct VirtualMemoryManager {
    root_table: *mut PageTable,
    depth: usize
}

unsafe impl Send for VirtualMemoryManager {}
unsafe impl Sync for VirtualMemoryManager {}

pub fn init() {
    let max_phys_addr = max_physical_address().align_up(PAGE_SIZE as u64);
    let page_count = max_phys_addr.as_u64() as usize / PAGE_SIZE;

    info!("Mapping [{}] pages for [{}] MiB of physical memory", page_count, max_phys_addr.as_u64() / 1024 / 1024);

    VIRTUAL_MEMORY_MANAGER.call_once(|| Mutex::new(VirtualMemoryManager::new(4)));

    let range = PageRange{ start: Page::containing_address(VirtAddr::zero()), end: Page::containing_address(VirtAddr::new(max_phys_addr.as_u64())) };
    map(range, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);

    let manager = VIRTUAL_MEMORY_MANAGER.get().unwrap().lock();
    let root_table_address = PhysFrame::from_start_address(PhysAddr::new(manager.root_table as u64)).unwrap();
    unsafe { Cr3::write(root_table_address, Cr3Flags::empty()) };
}

pub fn map(pages: PageRange, space: MemorySpace, flags: PageTableFlags) {
    let mut manager = VIRTUAL_MEMORY_MANAGER.get().unwrap().lock();
    manager.map(pages, space, flags);
}

fn page_table_index(virt_addr: VirtAddr, level: usize) -> PageTableIndex {
    return PageTableIndex::new_truncate((virt_addr.as_u64() >> 12 >> ((level as u8 - 1) * 9)) as u16);
}

impl VirtualMemoryManager {
    pub fn new(depth: usize) -> Self {
        let table_addr = physical::alloc(1, MemorySpace::Kernel);
        let root_table = table_addr.as_u64() as *mut PageTable;

        unsafe { root_table.as_mut().unwrap().zero(); }
        Self { root_table, depth }
    }

    fn map(&mut self, pages: PageRange, space: MemorySpace, flags: PageTableFlags) -> usize {
        let table: &mut PageTable = unsafe { self.root_table.as_mut().unwrap() };
        return VirtualMemoryManager::map_in_table(table, pages, space, flags, self.depth);
    }

    fn map_in_table(table: &mut PageTable, mut pages: PageRange, space: MemorySpace, flags: PageTableFlags, level: usize) -> usize {
        let mut total_allocated_pages: usize = 0;
        let start_index = usize::from(page_table_index(pages.start.start_address(), level));

        if level > 1 { // Calculate next level page table until level == 1
            for entry in table.iter_mut().skip(start_index) {
                let next_level_table;
                if entry.addr().is_null() { // Entry is empty -> Allocate new page frame
                    let phys_addr = physical::alloc(1, MemorySpace::Kernel);
                    entry.set_addr(phys_addr, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);

                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                    next_level_table.zero();
                } else {
                    next_level_table = unsafe { (entry.addr().as_u64() as *mut PageTable).as_mut().unwrap() };
                }

                let allocated_pages = VirtualMemoryManager::map_in_table(next_level_table, pages, space, flags, level - 1);
                pages = PageRange { start: pages.start + allocated_pages as u64, end: pages.end };
                total_allocated_pages = total_allocated_pages + allocated_pages;

                if pages.start >= pages.end {
                    break;
                }
            }
        } else { // Reached level 1 page table
            let alloc_count = min(pages.count(), 512 - start_index);

            match space {
                MemorySpace::Kernel => {
                    let mut frame_addr = PhysAddr::new(pages.start.start_address().as_u64());

                    for (index, entry) in table.iter_mut().skip(start_index).enumerate() {
                        if index >= start_index + alloc_count {
                            break;
                        }

                        entry.set_addr(frame_addr, flags);
                        frame_addr = frame_addr + PAGE_SIZE;
                    }
                },
                MemorySpace::User => {
                    for (index, entry) in table.iter_mut().skip(start_index).enumerate() {
                        if index >= start_index + alloc_count {
                            break;
                        }

                        let frame_addr = physical::alloc(1, MemorySpace::User);
                        entry.set_addr(frame_addr, flags);
                    }
                }
            }

            total_allocated_pages += alloc_count;
        }

        return total_allocated_pages;
    }
}