use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use spin::RwLock;
use crate::{memory, scheduler};
use crate::memory::r#virtual::{AddressSpace, VirtualMemoryArea, VmaType};

static PROCESSES: RwLock<ProcessManagement> = RwLock::new(ProcessManagement::new());
static PROCESS_ID_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn next_process_id() -> usize {
    PROCESS_ID_COUNTER.fetch_add(1, Relaxed)
}

pub fn create_process() -> Arc<Process> {
    let process = Arc::new(Process::new());
    PROCESSES.write().add(Arc::clone(&process));

    return process;
}

pub fn kernel_process() -> Option<Arc<Process>> {
    PROCESSES.read().kernel_process()
}

pub fn current_process() -> Arc<Process> {
    PROCESSES.read().current_process()
}

pub fn cleanup_exited_processes() {
    if let Some(mut management) = PROCESSES.try_write() {
        management.drop_exited_process();
    }
}

struct ProcessManagement {
    active_processes: Vec<Arc<Process>>,
    exited_processes: Vec<Arc<Process>>
}

impl ProcessManagement {
    const fn new() -> Self {
        Self { active_processes: Vec::new(), exited_processes: Vec::new() }
    }

    fn add(&mut self, process: Arc<Process>) {
        self.active_processes.push(process);
    }

    fn kernel_process(&self) -> Option<Arc<Process>> {
        match self.active_processes.get(0) {
            Some(kernel_process) => Some(Arc::clone(kernel_process)),
            None => None
        }
    }

    fn current_process(&self) -> Arc<Process> {
        if self.active_processes.len() > 1 {
            scheduler().current_thread().process()
        } else {
            kernel_process().unwrap()
        }
    }

    fn exit(&mut self, id: usize) {
        let index = self.active_processes.iter()
            .position(|process| process.id == id)
            .expect("Process: Trying to exit a non-existent process!");

        let process = Arc::clone(&self.active_processes[index]);
        self.active_processes.swap_remove(index);
        self.exited_processes.push(process);
    }

    fn drop_exited_process(&mut self) {
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
        PROCESSES.write().exit(self.id);
    }
}