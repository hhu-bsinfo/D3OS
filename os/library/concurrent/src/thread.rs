/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: thread                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for thread functions.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 31.8.2024, HHU              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec::Vec;
use core::ptr;
use syscall::{syscall, SystemCall};

pub struct Thread {
    id: usize,
}

impl Thread {
    const fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn join(&self) {
        let _ = syscall(SystemCall::ThreadJoin, &[self.id]);
    }
}

fn kickoff_user_thread(entry: fn()) {
    entry();
    exit();
}

pub fn create(entry: fn()) -> Option<Thread> {
    let res = syscall(SystemCall::ThreadCreate, &[kickoff_user_thread as usize,
        entry as usize,]);
    match res {
        Ok(id) => Some(Thread::new(id as usize)),
        Err(_) => None,
    }    
}

pub fn current() -> Option<Thread> {
    let res = syscall(SystemCall::ThreadId, &[]);
    match res {
        Ok(id) => Some(Thread::new(id as usize)),
        Err(_) => None,
    }    
}

#[allow(dead_code)]
pub fn switch() {
    let _ = syscall(SystemCall::ThreadSwitch, &[]);
}

#[allow(dead_code)]
pub fn sleep(ms: usize) {
    let _ = syscall(SystemCall::ThreadSleep, &[ms]);
}

pub fn exit() -> ! {
    let _ = syscall(SystemCall::ThreadExit, &[]);
    panic!("System call 'ThreadExit' has returned!")
}

pub fn start_application(name: &str, args: Vec<&str>) -> Option<Thread> {
    let res = syscall(SystemCall::ProcessExecuteBinary, &[name.as_bytes().as_ptr() as usize,
    name.len(),
    ptr::from_ref(&args) as usize,]);
    match res {
        Ok(id) => Some(Thread::new(id as usize)),
        Err(_) => None,
    }    
}
