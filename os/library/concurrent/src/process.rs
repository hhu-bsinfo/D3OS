/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for process functions.                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 31.8.2024, HHU              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use syscall::{syscall, SystemCall};

pub struct Process {
    id: usize,
}

impl Process {
    const fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(&self) -> usize {
        self.id
    }
}

pub fn current() -> Option<Process> {
    let res = syscall(SystemCall::ProcessId, &[]);
    match res {
        Ok(id) => Some(Process::new(id)),
        Err(_) => None,
    }    
}

pub fn exit() {
    syscall(SystemCall::ProcessExit, &[]).expect("Failed to exit process");
}
