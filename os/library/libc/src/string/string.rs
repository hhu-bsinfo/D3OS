/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::cmp::Ordering;
use core::ffi::{c_char, c_int, c_size_t, c_void, CStr};

// These functions are compiler builtins, so we do not need to implement them ourselves.
unsafe extern "C" {
    pub fn memcmp(s1: *const c_void, s2: *const c_void, n: c_size_t) -> c_int;
    pub fn memcpy(dest: *mut c_void, src: *const c_void, n: c_size_t) -> *mut c_void;
    pub fn memmove(dest: *mut c_void, src: *const c_void, n: c_size_t) -> *mut c_void;
    pub fn memset(dest: *mut c_void, c: c_int, n: c_size_t) -> *mut c_void;
    pub fn strlen(s: *const c_char) -> c_size_t;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    unsafe {
        let dst_len: usize = strlen(dst);
        strcpy(dst.add(dst_len), src)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcmp(dst: *const c_char, src: *const c_char) -> c_int {
    unsafe {
        let src = CStr::from_ptr(src);
        let dst = CStr::from_ptr(dst);

        match src.cmp(dst) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    unsafe {
        let src_len = strlen(src);

        dst.copy_from(src, src_len + 1); // + 1 for null terminator
        dst
    }
}

#[cfg(test)]
mod tests {
    use core::ffi::{c_char, c_int, c_void};
    use super::*;

    #[test_case]
    fn test_memcpy_char_array() {
        let src1 = ['a' as c_char, 'b' as c_char, 'c' as c_char];
        let src2 = ['d' as c_char, 'e' as c_char, 'f' as c_char];
        let mut dst = [0, 0, 0] as [c_char; 3];

        unsafe {
            memcpy(dst.as_mut_ptr() as *mut c_void, src1.as_ptr() as *mut c_void, 3 * size_of::<c_char>());
            assert_eq!(dst, src1);

            memcpy(dst.as_mut_ptr() as *mut c_void, src2.as_ptr() as *mut c_void, 3 * size_of::<c_char>());
            assert_eq!(dst, src2);
        }
    }

    #[test_case]
    fn test_memcpy_struct_array() {
        #[repr(C)]
        #[derive(Debug, PartialEq)]
        struct TestStruct {
            a: c_int,
            b: c_int,
        }

        let src =[
            TestStruct{ a: 1, b: 2 },
            TestStruct{ a: 3, b: 4 },
            TestStruct{ a: 5, b: 6 }
        ];

        let mut dst=[ TestStruct { a: 0, b: 0 },
            TestStruct { a: 0, b: 0 },
            TestStruct { a: 0, b: 0 }];

        unsafe {
            memcpy(dst.as_mut_ptr() as *mut c_void, src.as_ptr() as *mut c_void, 3 * size_of::<TestStruct>());
            assert_eq!(dst, src);
        }

    }

    #[test_case]
    fn test_memcpy_with_null_byte() {
        let src = [1, 2, 3] as [c_int; 3];
        let mut dst:[c_int; 3] = [0, 0, 0];

        unsafe {
            memcpy(dst.as_mut_ptr() as *mut c_void, src.as_ptr() as *mut c_void, 0);
            assert_eq!(dst, [0, 0, 0]);
        }

    }

    #[test_case]
    fn test_memset_char_array() {
        let mut dst = ['0' as c_char, '0' as c_char, '0' as c_char];
        let expected = ['a' as c_char, 'a' as c_char, 'a' as c_char];

        unsafe {
            memset(dst.as_mut_ptr() as *mut c_void, 'a' as c_int, 3 * size_of::<c_char>());
            assert_eq!(dst, expected);
        }
    }

    #[test_case]
    fn test_memset_int_array_1() {
        let mut dst = [0, 0, 0] as [c_int; 3];
        let expected = [0x01010101, 0x01010101, 0x01010101] as [c_int; 3];

        unsafe {
            // Set each byte to 1, which results in each int being 0x01010101
            memset(dst.as_mut_ptr() as *mut c_void, 1 as c_int, 3 * size_of::<c_int>());
            assert_eq!(dst, expected);
        }
    }

    #[test_case]
    fn test_memset_int_array_2() {
        let mut dst = [1, 2, 3] as [c_int; 3];
        let expected = [-1, -1, -1] as [c_int; 3];

        unsafe {
            // Set each byte to -1 (0xff), which results in each int being -1,
            // because -1 is represented as 0xffffffff in two's complement
            memset(dst.as_mut_ptr() as *mut c_void, -1 as c_int, 3 * size_of::<c_int>());
            assert_eq!(dst, expected);
        }
    }

    #[test_case]
    fn test_memcpy_int_array() {
        let src1 = [1, 2, 3] as [c_int; 3];
        let src2 = [4, 5, 6] as [c_int; 3];
        let mut dst = [0, 0, 0] as [c_int; 3];

        unsafe {
            memcpy(dst.as_mut_ptr() as *mut c_void, src1.as_ptr() as *mut c_void, 3 * size_of::<c_int>());
            assert_eq!(dst, src1);

            memcpy(dst.as_mut_ptr() as *mut c_void, src2.as_ptr() as *mut c_void, 3 * size_of::<c_int>());
            assert_eq!(dst, src2);
        }
    }
}