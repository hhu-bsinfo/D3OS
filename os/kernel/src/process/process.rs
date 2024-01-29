use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::Relaxed;
use spin::RwLock;
use crate::{memory, scheduler};
use crate::memory::r#virtual::AddressSpace;

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
    address_space: Arc<AddressSpace>
}

impl Process {
    fn new() -> Self {
        Self { id: next_process_id(), address_space: memory::r#virtual::create_address_space() }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn address_space(&self) -> Arc<AddressSpace> {
        Arc::clone(&self.address_space)
    }

    pub fn exit(&self) {
        PROCESSES.write().retain(|process| process.id != self.id);
    }
}