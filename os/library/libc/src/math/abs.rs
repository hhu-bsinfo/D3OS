/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::c_int;

#[unsafe(no_mangle)]
pub extern "C" fn abs(i: c_int) -> c_int {
    i.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_abs() {
        assert_eq!(abs(-1), 1);
        assert_eq!(abs(1), 1);
        assert_eq!(abs(0), 0);
    }
}