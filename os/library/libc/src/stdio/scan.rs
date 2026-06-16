use core::ffi::{c_char, c_int, VaList};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn vsscanf(_s: *const c_char, _format: *const c_char, _args: VaList) -> c_int {
    todo!()
}