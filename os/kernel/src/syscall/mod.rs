use log::debug;
use x86_64::structures::paging::PageTableFlags;
use crate::{scheduler, terminal};
use crate::memory::{MemorySpace, PAGE_SIZE};
use crate::memory::r#virtual::{VirtualMemoryArea, VmaType};
use crate::process::process::current_process;

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
    let terminal = terminal();
    for i in 0..length {
        unsafe { terminal.write_byte(buffer.offset(i as isize).read()) };
    }
}

#[no_mangle]
pub extern "C" fn sys_map_user_heap(size: usize) -> usize {
    debug!("Map User Heap!");
    let process = current_process();
    debug!("Got process!");
    let code_area = process.find_vma(VmaType::Code).expect("Process does not have code area!");
    debug!("Got code VMA!");
    let heap_start = code_area.end().align_up(PAGE_SIZE as u64);
    debug!("Got heap start!");
    let heap_area = VirtualMemoryArea::from_address(heap_start, size, VmaType::Heap);
    debug!("Got heap area!");

    process.address_space().map(heap_area.range(), MemorySpace::User, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE);
    debug!("Mapped heap!");
    process.add_vma(heap_area);
    debug!("Added heap VMA!");

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
pub extern "C" fn sys_thread_exit() {
    scheduler().exit();
}