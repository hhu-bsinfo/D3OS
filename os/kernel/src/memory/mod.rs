pub mod vmm;
pub mod frames;
pub mod pages;

pub mod nvmem;

pub mod kheap;
pub mod kstack;
pub mod acpi_handler;

#[derive(Clone, Copy)]
pub enum MemorySpace {
    Kernel,
    User
}

pub const PAGE_SIZE: usize = 0x1000;