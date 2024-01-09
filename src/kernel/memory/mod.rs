use x86_64::PhysAddr;

pub mod alloc;
pub mod physical;

pub const PAGE_SIZE: usize = 0x1000;
pub const KERNEL_PHYS_SIZE: PhysAddr = PhysAddr::new(0x4000000); // 64 MiB physical memory for the kernel