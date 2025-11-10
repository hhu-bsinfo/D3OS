#[repr(C)]
#[derive(Debug)]
pub struct ExceptionEntry {
    pub ftistn: u64,
    pub fixup: u64,
}

pub const SEC_FAULT: u8 = 0x01;

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum ExceptionSec {
    CopyException = 0x01
}

unsafe extern "C" {
    #[link_name = "__ex_entries"]
    static __EX_ENTRIES: usize;

    #[link_name = "__ex_table"]
    static __EX_TABLE: ExceptionEntry;

    #[link_name = "__load_user_byte"]
    pub fn load_user_byte(src: *const u8) -> u8;

    #[link_name = "__store_user_byte"]
    pub fn store_user_byte(dest: *mut u8, val: u8);
}

pub fn get_exception_table() -> &'static [ExceptionEntry] {
    unsafe {
        let start = &__EX_TABLE as *const ExceptionEntry;
        let count = __EX_ENTRIES;

        core::slice::from_raw_parts(start, count)
    }
}

pub fn fixup_fn(from_raw: u64) -> extern "C" fn() {
    unsafe { core::mem::transmute::<u64, extern "C" fn()>(from_raw) }
}
