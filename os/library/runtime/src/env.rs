use alloc::string::{String, ToString};
use core::fmt::Display;
use core::ptr;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;

// Duplicated from 'kernel/src/consts.rs'
const USER_SPACE_START: usize = 0x10000000000;
const USER_SPACE_CODE_START: usize = USER_SPACE_START;
const USER_SPACE_ENV_START: usize = USER_SPACE_CODE_START + 0x40000000;
const USER_SPACE_ARG_START: usize = USER_SPACE_ENV_START;

const ARGC_PTR: *const usize = USER_SPACE_ARG_START as *mut usize;
const ARGV_PTR: *const *const Argument = (USER_SPACE_ARG_START + size_of::<usize>()) as *const *const Argument;

pub fn args() -> Args {
    Args::new()
}

#[repr(C, packed)]
struct Argument {
    len: usize,
    data: u8
}

impl Display for Argument {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let slice = slice_from_raw_parts(ptr::from_ref(&self.data), self.len);
        let string = unsafe { from_utf8(&*slice).unwrap() };

        f.write_str(string)
    }
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
            let argc = *ARGC_PTR ;
            if self.index >= argc {
                return None;
            }

            let arg_ptr = *ARGV_PTR.offset(self.index as isize);
            let arg = arg_ptr.as_ref()?;
            self.index += 1;

            Some(arg.to_string())
        }
    }
}