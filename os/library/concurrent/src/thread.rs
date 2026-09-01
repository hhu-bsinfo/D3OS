/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: thread                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for thread functions.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, Niklas Sombert, 7.4.26, HHU ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::{boxed::Box, collections::btree_map::BTreeMap};
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicUsize, Ordering};
use chrono::TimeDelta;
use time::systime;
use spin::Mutex;
use syscall::{SystemCall, syscall,return_vals::Errno};

static NEXT_FUNCTION_ID: AtomicUsize = AtomicUsize::new(0);
/// These are functions to be spawned in new threads.
static FUNCTIONS: Mutex<BTreeMap<usize, Box<dyn FnOnce() + Send + 'static>>> = Mutex::new(BTreeMap::new());

pub struct Thread {
    id: usize,
}

#[repr(C, packed)]
pub struct ThreadEnvironment {
    start_time: TimeDelta,
}

impl Thread {
    const fn new(id: usize) -> Self {
        Self { id }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn join(&self) -> Result<usize, Errno> {
        syscall(SystemCall::ThreadJoin, &[self.id])
    }

    pub fn kill(&self) {
        let _ = syscall(SystemCall::ThreadKill, &[self.id]);
    }

    pub fn start_time(&self) -> TimeDelta {
        let thread_env = thread_environment();
        thread_env.start_time
    }
}

pub fn thread_environment() -> &'static mut ThreadEnvironment {
    let thread_env: *mut ThreadEnvironment;

    unsafe {
        asm!(
        "rdfsbase {0}",
        out(reg) thread_env,
        );

        &mut *thread_env
    }
}

pub fn init_thread_environment() {
    let thread_env = Box::new(ThreadEnvironment {
        start_time: systime(),
    });

    let thread_env_ptr = Box::into_raw(thread_env);
    unsafe {
        asm!(
        "wrfsbase {0}",
        in(reg) thread_env_ptr,
        );
    }
}

extern "sysv64" fn kickoff_user_thread(func_id: usize) {
    // set up the thread environment, which is stored at FS:0
    init_thread_environment();

    // pop the function and execute it
    let function = {
        let mut functions = FUNCTIONS.lock();
        functions.remove(&func_id).expect("lost function during thread spawn")
    };
    function();
    exit();
}

pub fn create(entry: impl FnOnce() + Send + 'static) -> Option<Thread> {
    // impl Fn() allows us to work on closures, but we can't pass this through the syscall.
    // So, we generate a unique ID and save it with that ID in our process.
    // The syscall gets the ID and passes it back to the new thread.
    // From there, we can retrieve the closure.
    // 
    // This could be faster if we just passsed raw pointers to boxes,
    // but that would be unsafe.
    let mut functions = FUNCTIONS.lock();
    let func_id = NEXT_FUNCTION_ID.fetch_add(1, Ordering::SeqCst);
    functions.insert(func_id, Box::new(entry));
    
    let res = syscall(SystemCall::ThreadCreate, &[
        kickoff_user_thread as *const () as usize, func_id,
    ]);
    match res {
        Ok(id) => Some(Thread::new(id)),
        Err(_) => None,
    }    
}

pub fn current() -> Option<Thread> {
    let res = syscall(SystemCall::ThreadId, &[]);
    match res {
        Ok(id) => Some(Thread::new(id)),
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

pub fn count() -> usize {
    syscall(SystemCall::ThreadCount, &[]).unwrap_or(0)
}

pub fn start_application(name: &str, args: Vec<&str>) -> Option<Thread> {
    let res = syscall(SystemCall::ProcessExecuteBinary, &[name.as_bytes().as_ptr() as usize,
    name.len(),
    ptr::from_ref(&args) as usize,]);
    match res {
        Ok(id) => Some(Thread::new(id)),
        Err(_) => None,
    }    
}
