

pub const MAIN_USER_STACK_START: usize = 0x400000000000;  // 10 TiB
pub const MAX_USER_STACK_SIZE: usize = 0x40000000;  // 1 GiB
pub const KERNEL_STACK_PAGES: usize = 64;
pub const STACK_ENTRY_SIZE: usize = 8;  
