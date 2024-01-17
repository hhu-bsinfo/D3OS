use spin::Once;
use x86_64::structures::paging::PhysFrame;

pub mod alloc;
pub mod physical;
pub mod r#virtual;

#[derive(Clone, Copy)]
pub enum MemorySpace {
    Kernel,
    User
}

pub const PAGE_SIZE: usize = 0x1000;
pub static KERNEL_PHYS_LIMIT: Once<PhysFrame> = Once::new();