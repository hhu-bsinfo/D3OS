use alloc::rc::Rc;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use x86_64::structures::paging::PageTableFlags;
use crate::{initrd, scheduler, terminal};
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::memory::r#virtual::{VirtualMemoryArea, VmaType};
use crate::process::process::current_process;
use crate::process::thread::Thread;

pub mod syscall_dispatcher;

#[no_mangle]
pub extern "C" fn sys_read() -> u64 {
    let terminal = terminal();
    match terminal.read_byte() {
        -1 => panic!("Input stream closed!"),
        c => c as u64
    }
}

#[no_mangle]
pub extern "C" fn sys_write(buffer: *const u8, length: usize) {
    let string = from_utf8(unsafe { slice_from_raw_parts(buffer, length).as_ref().unwrap() }).unwrap();
    let terminal = terminal();
    terminal.write_str(string);
}

#[no_mangle]
pub extern "C" fn sys_map_user_heap(size: usize) -> usize {
    let process = current_process();
    let code_area = process.find_vma(VmaType::Code).expect("Process does not have code area!");
    let heap_start = code_area.end().align_up(PAGE_SIZE as u64);
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);

    process.address_space().map(heap_area.range(), MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
    process.add_vma(heap_area);

    return heap_start.as_u64() as usize;
}

#[no_mangle]
pub extern "C" fn sys_process_id() -> usize {
    current_process().id()
}

#[no_mangle]
pub extern "C" fn sys_thread_id() -> usize {
    scheduler().current_thread().id()
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
pub extern "C" fn sys_thread_join(id: usize) {
    scheduler().join(id);
}

#[no_mangle]
pub extern "C" fn sys_thread_exit() {
    scheduler().exit();
}

#[no_mangle]
pub extern "C" fn sys_application_start(name_buffer: *const u8, name_length: usize) -> usize {
    let app_name = from_utf8(unsafe { slice_from_raw_parts(name_buffer, name_length).as_ref().unwrap() }).unwrap();
    match initrd().entries().find(|entry| entry.filename().as_str() == app_name) {
        Some(app) => {
            let thread = Thread::new_user_thread(app.data());
            scheduler().ready(Rc::clone(&thread));
            thread.id()
        }
        None => 0
    }
}