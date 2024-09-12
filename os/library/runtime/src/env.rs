use alloc::string::{String, ToString};
use core::ffi::CStr;
use core::ptr::slice_from_raw_parts;
use stream::strlen;

// Duplicated from 'kernel/src/consts.rs'
const USER_SPACE_START: usize = 0x10000000000;
const USER_SPACE_CODE_START: usize = USER_SPACE_START;
const USER_SPACE_ENV_START: usize = USER_SPACE_CODE_START + 0x40000000;
const USER_SPACE_ARG_START: usize = USER_SPACE_ENV_START;

pub(crate) const ARGC_PTR: *const usize = USER_SPACE_ARG_START as *const usize;
pub(crate) const ARGV_PTR: *const *const u8 = (USER_SPACE_ARG_START + size_of::<*const usize>()) as *const *const u8;

pub fn args() -> Args {
    Args::new()
}

pub struct Args {
    index: usize
}

impl Args {
    fn new() -> Self {
        Args { index: 0 }
    }
}

impl Iterator for Args {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let argc = *ARGC_PTR;
            if self.index >= argc {
                return None;
            }

            let arg = *ARGV_PTR.add(self.index);
            let len = strlen(arg);
            self.index += 1;

            CStr::from_bytes_with_nul(slice_from_raw_parts(arg, len + 1).as_ref()?)
                .map(|cstr| cstr.to_str().expect("Invalid UTF-8 in argument").to_string())
                .ok()
        }
    }
}