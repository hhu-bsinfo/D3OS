#![no_std]

use core::{ptr, slice};

use bitflags::bitflags;
use syscall::{return_vals::{Errno}, syscall, SystemCall};

pub const PAGE_SIZE: usize = 0x1000;

bitflags! {
    pub struct MmapFlags: u8 {
        const ANONYMOUS = 0x01;
        const POPULATE  = 0x02; // fault in
        const ALLOC_AT  = 0x04; // instead of passing 0 as start, this marks if it has a start address or not
    }
}

pub fn mmap(start: usize, size: usize, options: MmapFlags) -> Result<&'static mut [u8], Errno> {
    let ptr = syscall(SystemCall::MapMemory, &[
        start,
        size,
        options.bits() as usize
    ])?;

    unsafe { Ok(slice::from_raw_parts_mut(ptr as *mut u8, size)) }
}