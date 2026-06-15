/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::c_int;

#[repr(C)]
pub struct tm {
    pub tm_sec: c_int, // seconds after the minute [0-60]
    pub tm_min: c_int, // minutes after the hour [0-59]
    pub tm_hour: c_int, // hours since midnight [0-23]
    pub tm_mday: c_int, // day of the month [1-31]
    pub tm_mon: c_int, // months since January [0-11]
    pub tm_year: c_int, // years since 1900
    pub tm_wday: c_int, // days since Sunday [0-6]
    pub tm_yday: c_int, // days since January 1 [0-365]
    pub tm_isdst: c_int // Daylight Saving Time flag
}