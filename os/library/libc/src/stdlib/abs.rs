/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - stdlib.h: abs(), abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 *   - time.h: struct tm
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::c_int;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn abs(n: c_int) -> c_int {
    n.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_abs() {
        unsafe {
            assert_eq!(abs(10), 10);
            assert_eq!(abs(-10), 10);
            assert_eq!(abs(0), 0);
        }
    }
}