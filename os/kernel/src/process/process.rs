use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use spin::RwLock;
use crate::{memory, scheduler};
use crate::memory::r#virtual::{AddressSpace, VirtualMemoryArea, VmaType};

static PROCESSES: RwLock<Vec<Arc<Process>>> = RwLock::new(Vec::new());
static PROCESS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next_process_id() -> usize {
    PROCESS_ID_COUNTER.fetch_add(1, Relaxed)
}

pub fn create_process() -> Arc<Process> {
    let process = Arc::new(Process::new());
    PROCESSES.write().push(Arc::clone(&process));

    return process;
}

pub fn kernel_process() -> Option<Arc<Process>> {
    match PROCESSES.read().get(0) {
        Some(kernel_process) => Some(Arc::clone(kernel_process)),
        None => None
    }
}

pub fn current_process() -> Arc<Process> {
    if PROCESSES.read().len() > 1 {
        scheduler().current_thread().process()
    } else {
        kernel_process().unwrap()
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
            self.address_space.unmap(vma.range());
        }
    }
}

impl Process {
    fn new() -> Self {
        Self { id: next_process_id(), address_space: memory::r#virtual::create_address_space(), memory_areas: RwLock::new(Vec::new()) }
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
        PROCESSES.write().retain(|process| process.id != self.id);
    }
}