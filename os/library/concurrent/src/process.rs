/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: process                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for process functions.                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 26.12.2025, HHU             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use core::ptr;
use syscall::{SystemCall, return_vals::Errno, syscall};
use uuid::Uuid;

pub struct Process {
    id: Uuid,
}

impl Process {
    const fn new(id: Uuid) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> Uuid {
        self.id
    }
}

pub fn current() -> Process {
    let mut id: u128 = 0;
    syscall(SystemCall::ProcessId, &[ptr::from_mut(&mut id) as usize])
        .expect("failed to get process id");
    Process::new(Uuid::from_u128(id))
}

pub fn exit() {
    syscall(SystemCall::ProcessExit, &[]).expect("Failed to exit process");
}

pub fn count() -> usize {
    syscall(SystemCall::ProcessCount, &[]).unwrap_or(0)
}

pub fn ps(buf: &mut [u8]) -> Result<usize, Errno> {
      syscall(SystemCall::ProcessStatus, &[
        buf.as_mut_ptr() as usize,
        buf.len(),
    ])
}

