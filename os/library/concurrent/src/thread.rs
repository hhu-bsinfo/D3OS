use syscall::{syscall0, syscall1, syscall2, SystemCall};

pub struct Thread {
    id: usize
}

impl Thread {
    const fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn join(&self) {
        syscall1(SystemCall::ThreadJoin, self.id);
    }
}

fn kickoff_user_thread(entry: fn()) {
    entry();
    exit();
}

pub fn create(entry: fn()) -> Thread {
    let id = syscall2(SystemCall::ThreadCreate, kickoff_user_thread as usize, entry as usize);
    Thread::new(id)
}

pub fn current() -> Thread {
    let id = syscall0(SystemCall::ThreadId);
    Thread::new(id)
}

#[allow(dead_code)]
pub fn switch() {
    syscall0(SystemCall::ThreadSwitch);
}

#[allow(dead_code)]
pub fn sleep(ms: usize) {
    syscall1(SystemCall::ThreadSleep, ms);
}

pub fn exit() -> ! {
    syscall0(SystemCall::ThreadExit);
    panic!("System call 'ThreadExit' has returned!")
}

pub fn start_application(name: &str) -> Option<Thread> {
    match syscall2(SystemCall::ProcessExecuteBinary, name.as_bytes().as_ptr() as usize, name.len()) {
        0 => None,
        id => Some(Thread::new(id))
    }
}
