use syscall::{syscall0, SystemCall};

pub struct Process {
    id: usize
}

impl Process {
    const fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

pub fn current() -> Process {
    let id = syscall0(SystemCall::ProcessId);
    Process::new(id)
}