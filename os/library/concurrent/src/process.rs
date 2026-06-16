/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for process functions.                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 26.12.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use syscall::{SystemCall, return_vals::Errno, syscall};

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

pub fn count() -> usize {
    match syscall(SystemCall::ProcessCount, &[]) {
        Ok(count) => count,
        Err(_) => 0,
    }
    
}

pub fn ps(buf: &mut [u8]) -> Result<usize, Errno> {
      syscall(SystemCall::ProcessStatus, &[
        buf.as_mut_ptr() as usize,
        buf.len(),
    ])
}

