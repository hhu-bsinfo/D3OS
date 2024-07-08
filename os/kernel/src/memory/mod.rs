pub mod alloc;
pub mod physical;
pub mod r#virtual;
pub mod nvmem;
pub mod cxl;

#[derive(Clone, Copy)]
pub enum MemorySpace {
    Kernel,
    User
}

pub const PAGE_SIZE: usize = 0x1000;