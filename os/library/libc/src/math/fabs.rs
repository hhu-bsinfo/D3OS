/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::c_double;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn fabs(a: c_double) -> c_double {
    a.abs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fabs() {
        unsafe {
            assert_eq!(fabs(-1.5), 1.5);
            assert_eq!(fabs(1.5), 1.5);
            assert_eq!(fabs(0.0), 0.0);
        }
    }
}