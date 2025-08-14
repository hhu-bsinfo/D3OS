/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

// The test environment calls abort() when a non-unwinding panic occurs.
// In this case, we do not want this to be called, but the environment's own abort() function.
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn abort() -> ! {
    panic!("libc abort called!");
}