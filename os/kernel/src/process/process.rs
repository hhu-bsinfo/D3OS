use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use spin::RwLock;
use x86_64::structures::paging::{Page, PageTableFlags};
use x86_64::structures::paging::page::PageRange;
use x86_64::VirtAddr;
use crate::{ process_manager, scheduler};
use crate::memory::MemorySpace;
use crate::memory::physical::phys_limit;
use crate::memory::r#virtual::{AddressSpace, VirtualMemoryArea, VmaType};

static PROCESS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next_process_id() -> usize {
    PROCESS_ID_COUNTER.fetch_add(1, Relaxed)
}

pub struct ProcessManager {
    active_processes: Vec<Arc<Process>>,
    exited_processes: Vec<Arc<Process>>
}

impl ProcessManager {
    pub const fn new() -> Self {
        Self { active_processes: Vec::new(), exited_processes: Vec::new() }
    }

    pub fn create_process(&mut self) -> Arc<Process> {
        let address_space = match self.kernel_process() {
            Some(kernel_process) => { // Create user address space
                Arc::new(AddressSpace::from_other(&kernel_process.address_space()))
            }
            None => { // Create kernel address space
                let address_space = AddressSpace::new(4);
                let max_phys_addr = phys_limit().start_address();
                let range = PageRange { start: Page::containing_address(VirtAddr::zero()), end: Page::containing_address(VirtAddr::new(max_phys_addr.as_u64())) };

                address_space.map(range, MemorySpace::Kernel, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
                Arc::new(address_space)
            }
        };

        let process = Arc::new(Process::new(address_space));
        self.active_processes.push(Arc::clone(&process));

        return process;
    }

    pub fn active_process_ids(&self) -> Vec<usize> {
        self.active_processes.iter().map(|process| process.id()).collect()
    }

    pub fn kernel_process(&self) -> Option<Arc<Process>> {
        match self.active_processes.get(0) {
            Some(kernel_process) => Some(Arc::clone(kernel_process)),
            None => None
        }
    }

    pub fn current_process(&self) -> Arc<Process> {
        if self.active_processes.len() > 1 {
            scheduler().current_thread().process()
        } else {
            self.kernel_process().unwrap()
        }
    }

    pub fn exit(&mut self, id: usize) {
        let index = self.active_processes.iter()
            .position(|process| process.id == id)
            .expect("Process: Trying to exit a non-existent process!");

        let process = Arc::clone(&self.active_processes[index]);
        self.active_processes.swap_remove(index);
        self.exited_processes.push(process);
    }

    pub fn drop_exited_process(&mut self) {
        self.exited_processes.clear();
    }
}

pub struct Process {
    id: usize,
    address_space: Arc<AddressSpace>,
    memory_areas: RwLock<Vec<VirtualMemoryArea>>
}

impl Drop for Process {
    fn drop(&mut self) {
        for vma in self.memory_areas.read().iter() {
            self.address_space.unmap(vma.range(), true);
        }
    }
}

impl Process {
    fn new(address_space: Arc<AddressSpace>) -> Self {
        Self { id: next_process_id(), address_space, memory_areas: RwLock::new(Vec::new()) }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn address_space(&self) -> Arc<AddressSpace> {
        Arc::clone(&self.address_space)
    }

    pub fn add_vma(&self, new_area: VirtualMemoryArea) {
        let mut areas = self.memory_areas.write();
        match areas.iter().find(|area| area.overlaps_with(&new_area)) {
            Some(_) => panic!("Process: Trying to add a VMA, which overlaps with an existing one!"),
            None => areas.push(new_area)
        }
    }

    pub fn find_vma(&self, typ: VmaType) -> Option<VirtualMemoryArea> {
        let areas = self.memory_areas.read();
        match areas.iter().find(|area| area.typ() == typ) {
            Some(area) => Some(*area),
            None => None
        }
    }

    pub fn update_vma(&self, vma: VirtualMemoryArea, update: impl Fn(&mut VirtualMemoryArea)) {
        let mut areas = self.memory_areas.write();
        match areas.iter_mut().find(|area| **area == vma) {
            Some(area) => update(area),
            None => panic!("Trying to update a non-existent VMA!")
        }
    }

    pub fn exit(&self) {
        process_manager().write().exit(self.id);
    }
}