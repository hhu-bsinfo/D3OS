/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::{c_char, c_int, c_long, CStr};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atoi(str: *const c_char) -> c_int {
    unsafe {
        atol(str) as c_int
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atol(str: *const c_char) -> c_long {
    unsafe {
        strtol(str, core::ptr::null_mut(), 10)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtol(str: *const c_char, endptr: *mut *mut c_char, base: c_long) -> c_long {
    // Closure to determine if a character is invalid for atoi.
    let invalid_char = |c : char| !c.is_digit(10) && c != '+' && c != '-';

    unsafe {
        let num_str = CStr::from_ptr(str)
            // Convert C string to Rust string
            .to_str()
            .expect("strtol: Invalid string")
            // Remove leading whitespace characters
            .trim_start()
            // Remove invalid trailing characters
            .split(invalid_char)
            .next()
            .unwrap();

        if !endptr.is_null() {
            // If endptr is not null, set it to the end of the parsed number
            let end = str as *mut c_char;
            *endptr = end.add(num_str.len())
        }

        c_long::from_str_radix(num_str, base as u32)
            .expect("strtol: Failed to parse string as integer")
    }
}

#[cfg(test)]
mod tests {
    use alloc::ffi::CString;
    use super::*;

    #[test]
    fn test_atoi() {
        unsafe {
            let result = atoi(CString::new("123").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 123);

            let result = atoi(CString::new("-123").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -1234);

            let result = atoi(CString::new("+14124").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 14124);

            let result = atoi(CString::new("-134125").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -134125);
        }
    }

    #[test]
    fn test_atoi_space() {
        unsafe {
            let result = atoi(CString::new(" 123").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 123);

            let result = atoi(CString::new("  -123").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -123);

            let result = atoi(CString::new("+123   ").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 123);

            let result = atoi(CString::new("    -123   ").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -123);

            let result = atoi(CString::new(" 1 23  ").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 1);
        }
    }

    #[test]
    #[should_panic]
    fn test_atoi_empty() {
        unsafe {
            let _result = atoi(CString::new("").unwrap().as_c_str().as_ptr());
        }
    }

    #[test]
    #[should_panic]
    fn test_atoi_negative_wrong_format() {
        unsafe {
            let _result = atoi(CString::new("--123").unwrap().as_c_str().as_ptr());
        }
    }

    #[test]
    #[should_panic]
    fn test_text() {
        unsafe {
            let _result = atoi(CString::new("Hello!").unwrap().as_c_str().as_ptr());
        }
    }

    #[test]
    fn test_overflow() {
        unsafe {
            let _result = atoi(CString::new("-2147483648").unwrap().as_c_str().as_ptr());
        }
    }
}