/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: thread                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for thread functions.                                  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Fabian Ruhland, Michael Schoettner, 31.8.2024, HHU              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::arch::asm;
use core::ptr;
use syscall::{syscall, SystemCall};
//use chrono::TimeDelta; // commented out because standard library depends now on thread.rs
//use time::systime;     // was used for start_time which needs chrono::TimeDelta

pub struct Thread {
    id: usize,
}

//#[repr(C, packed)]
pub struct ThreadEnvironment {
    //start_time: TimeDelta, // see line 14
    thread_local_storage: [*mut u8; 1024], // array for TLS
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

    pub fn kill(&self) {
        let _ = syscall(SystemCall::ThreadKill, &[self.id]);
    }

    // does not work without chrono::Timedelta; was only used by application "threadtest"
    /*pub fn start_time(&self) -> TimeDelta {
        let thread_env = thread_environment();
        thread_env.start_time
    }*/
}

// set TLS value, returns nothing (used by rust/libary/std/src/sys/thread_local/key/d3os.rs)
pub fn set_tls_value(key: usize, value: *mut u8) {
    let thread_env = thread_environment();
    thread_env.thread_local_storage[key] = value;
}

// get TLS value, returns value or default (used by rust/libary/std/src/sys/thread_local/key/d3os.rs)
pub fn get_tls_value(key: usize) -> *mut u8 {
    let thread_env = thread_environment();
    thread_env.thread_local_storage[key]
}

// returns a thread environment
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
        //start_time: systime(), // see line 14
        thread_local_storage: [Default::default(); 1024], // initialise TLS with 1024 null_mut pointers
    });

    let thread_env_ptr = Box::into_raw(thread_env);
    unsafe {
        asm!(
        "wrfsbase {0}",
        in(reg) thread_env_ptr,
        );
    }
}

extern "sysv64" fn kickoff_user_thread(entry: extern "sysv64" fn()) {
    // set up the thread environment, which is stored at FS:0
    init_thread_environment();

    // entry has no parameters, so we don't really need to ensure the calling convention
    entry();
    exit();
}

pub fn create(entry: fn()) -> Option<Thread> {
    let res = syscall(SystemCall::ThreadCreate, &[kickoff_user_thread as usize,
        entry as usize,]);
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
    syscall(SystemCall::ThreadCount, &[]).unwrap_or_else(|_| 0)
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
