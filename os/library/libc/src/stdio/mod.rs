use core::ffi::c_size_t;

pub mod print;
pub mod scan;
pub mod file;

type FILE = c_size_t;

#[unsafe(no_mangle)]
static stdout: FILE = 0x00;
#[unsafe(no_mangle)]
static stderr: FILE = 0x01;