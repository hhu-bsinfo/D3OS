use crate::scheduler;

pub mod syscall_dispatcher;

#[no_mangle]
pub extern "C" fn sys_print(c: u32) {
    print!("{}", char::from_u32(c).unwrap());
}

#[no_mangle]
pub extern "C" fn sys_thread_switch() {
    scheduler().switch_thread();
}

#[no_mangle]
pub extern "C" fn sys_thread_sleep(ms: usize) {
    scheduler().sleep(ms);
}

#[no_mangle]
pub extern "C" fn sys_thread_exit() {
    scheduler().exit();
}