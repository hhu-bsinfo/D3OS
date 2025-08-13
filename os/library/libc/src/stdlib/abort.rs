/*
 * The C standard library is based on a bachelor's thesis, written by Gökhan Cöpcü.
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

#[unsafe(no_mangle)]
pub unsafe extern "C" fn abort() -> ! {
    panic!("libc abort called!");
}