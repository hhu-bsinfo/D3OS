pub mod vmm;
pub mod vma;
pub mod pages;
pub mod frames;
pub mod frames_lf;

pub mod nvmem;
pub mod dram;


pub mod heap;
pub mod stack;
pub mod acpi_handler;

#[derive(PartialEq)]
#[derive(Clone, Copy, Debug)]
pub enum MemorySpace {
    Kernel,
    User
}

pub const PAGE_SIZE: usize = 0x1000;