/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - stdlib.h: abs(), abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 *   - time.h: struct tm
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::{c_char, c_int};

// The test environment calls abort() when a non-unwinding panic occurs.
// In this case, we do not want this to be called, but the environment's own abort() function.
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn abort() -> ! {
    todo!()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn exit() -> ! {
    todo!()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn system(_command: *const c_char) -> c_int {
    todo!()
}