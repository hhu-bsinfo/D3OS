/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::{c_int, c_size_t, c_void};
use core::ptr;
use core::slice::from_raw_parts;
use core::ptr::addr_of;

type Comparator = unsafe extern "C" fn(*const c_void, *const c_void) -> c_int;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn bsearch(key: *const c_void, base: *const c_void, count: c_size_t, size: c_size_t, comp: Comparator) -> *const c_void {
    if base == ptr::null() || count == 0 || size == 0 {
        return ptr::null();
    }

    let mut left = 0;
    let mut right = count - 1;

    unsafe {
        let base = from_raw_parts(base as *mut u8, count as usize * size as usize);

        while left <= right {
            let mid = (left + right) / 2;
            let mid_ptr = addr_of!(base[mid * size]) as *const c_void;

            if comp(key, mid_ptr) == 0 {
                return mid_ptr;
            } else if comp(key, mid_ptr) > 0 {
                left = mid + 1;
            } else {
                right = mid - 1;
            }
        }

        ptr::null()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::c_char;
    use crate::stdlib::{comp_char, comp_int};

    #[test]
    fn test_bsearch() {
        let arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as [c_int; 10];
        let key = 4 as c_int;

        unsafe {
            let result = bsearch(
                ptr::from_ref(&key) as *const c_void,
                arr.as_ptr() as *const c_void,
                arr.len() as c_size_t,
                size_of::<c_int>() as c_size_t,
                comp_int
            );

            assert_eq!(*(result as *const c_int), key);
        }
    }
    #[test]
    fn test_bsearch_char() {
        let arr = [
            'a' as c_char,
            'b' as c_char ,
            'c' as c_char,
            'd' as c_char,
            'e' as c_char,
            'f' as c_char,
            'g' as c_char,
            'h' as c_char,
            'i' as c_char,
            'j' as c_char
        ];
        let key = 'c' as c_char;

        unsafe {
            let result = bsearch(
                ptr::from_ref(&key) as *const c_void,
                arr.as_ptr() as *const c_void,
                arr.len() as c_size_t,
                size_of::<c_char>() as c_size_t,
                comp_char
            );

            assert_eq!(*(result as *const c_char), key);
        }
    }

    #[test]
    fn test_bsearch_notfound() {
        let arr = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10] as [c_int; 10];
        let key = 11 as c_int;

        unsafe {
            let result = bsearch(
                ptr::from_ref(&key) as *const c_void,
                arr.as_ptr() as *const c_void,
                arr.len() as c_size_t,
                size_of::<c_int>() as c_size_t,
                comp_int
            );

            assert!(result.is_null());
        }
    }
}
