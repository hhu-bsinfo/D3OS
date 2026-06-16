/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::cmp::min;
use core::ffi::{c_char, c_int, c_size_t, c_void};
use crate::ctype::ctype::toupper;
use crate::stdlib::memory::malloc;

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
pub unsafe extern "C" fn strcmp(mut lhs: *const c_char, mut rhs: *const c_char) -> c_int {
    unsafe {
        loop {
            let lch = *lhs;
            let rch = *rhs;

            if lch == 0 && rch == 0 {
                return 0;
            }

            if lch < rch {
                return -1;
            } else if lch > rch {
                return 1;
            }

            lhs = lhs.add(1);
            rhs = rhs.add(1);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncmp(mut lhs: *const c_char, mut rhs: *const c_char, count: c_size_t) -> c_int {
    unsafe {
        for _ in 0..count {
            let lch = *lhs;
            let rch = *rhs;

            if lch == 0 && rch == 0 {
                return 0;
            }

            if lch < rch {
                return -1;
            } else if lch > rch {
                return 1;
            }

            lhs = lhs.add(1);
            rhs = rhs.add(1);
        }

        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcasecmp(mut lhs: *const c_char, mut rhs: *const c_char) -> c_int {
    unsafe {
        loop {
            let lch = toupper(*lhs as c_int) as c_char;
            let rch = toupper(*rhs as c_int) as c_char;

            if lch == 0 && rch == 0 {
                return 0;
            }

            if lch < rch {
                return -1;
            } else if lch > rch {
                return 1;
            }

            lhs = lhs.add(1);
            rhs = rhs.add(1);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncasecmp(mut lhs: *const c_char, mut rhs: *const c_char, count: c_size_t) -> c_int {
    unsafe {
        for _ in 0..count {
            let lch = toupper(*lhs as c_int) as c_char;
            let rch = toupper(*rhs as c_int) as c_char;

            if lch == 0 && rch == 0 {
                return 0;
            }

            if lch < rch {
                return -1;
            } else if lch > rch {
                return 1;
            }

            lhs = lhs.add(1);
            rhs = rhs.add(1);
        }

        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strncpy(dest: *mut c_char, src: *const c_char, count: c_size_t) -> *mut c_char {
    unsafe {
        let src_len = strlen(src);
        let bytes_to_copy = min(src_len, count);

        dest.copy_from(src, bytes_to_copy as usize);
        if count > src_len {
            let padding_size = count - src_len;
            dest.add(src_len as usize).write_bytes(0, padding_size as usize);
        }
    }

    dest
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char {
    unsafe {
        let src_len = strlen(src);

        dst.copy_from(src, src_len + 1); // + 1 for null terminator
        dst
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strdup(str1: *const c_char) -> *mut c_char {
    unsafe {
        let len = strlen(str1);
        let dup = malloc(len + 1) as *mut u8;

        if !dup.is_null() {
            dup.copy_from_nonoverlapping(str1 as *const u8, len + 1);
        }

        dup as *mut c_char
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strchr(str: *const c_char, ch: c_int) -> *mut c_char {
    let mut ptr = str;

    unsafe {
        loop {
            let current_char = *ptr;
            if current_char == (ch as c_char) {
                return ptr as *mut c_char;
            }
            if current_char == 0 {
                return core::ptr::null_mut();
            }

            ptr = ptr.add(1);
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strrchr(str: *const c_char, ch: c_int) -> *mut c_char {
    let mut result: *mut c_char = core::ptr::null_mut();
    let mut ptr = str;

    unsafe {
        loop {
            let current_char = *ptr;
            if current_char == (ch as c_char) {
                result = ptr as *mut c_char;
            }
            if current_char == 0 {
                break;
            }

            ptr = ptr.add(1);
        }
    }

    result
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strstr(str: *const c_char, substr: *const c_char) -> *mut c_char {
    let substr_len = unsafe { strlen(substr) };
    if substr_len == 0 {
        return str as *mut c_char;
    }

    let mut ptr = str;

    unsafe {
        while *ptr != 0 {
            if memcmp(ptr as *const c_void, substr as *const c_void, substr_len) == 0 {
                return ptr as *mut c_char;
            }

            ptr = ptr.add(1);
        }
    }

    core::ptr::null_mut()
}

#[cfg(test)]
mod tests {
    use core::ffi::{c_char, c_int, c_void};
    use super::*;

    #[test]
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

    #[test]
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

    #[test]
    fn test_memcpy_with_null_byte() {
        let src = [1, 2, 3] as [c_int; 3];
        let mut dst:[c_int; 3] = [0, 0, 0];

        unsafe {
            memcpy(dst.as_mut_ptr() as *mut c_void, src.as_ptr() as *mut c_void, 0);
            assert_eq!(dst, [0, 0, 0]);
        }

    }

    #[test]
    fn test_memset_char_array() {
        let mut dst = ['0' as c_char, '0' as c_char, '0' as c_char];
        let expected = ['a' as c_char, 'a' as c_char, 'a' as c_char];

        unsafe {
            memset(dst.as_mut_ptr() as *mut c_void, 'a' as c_int, 3 * size_of::<c_char>());
            assert_eq!(dst, expected);
        }
    }

    #[test]
    fn test_memset_int_array_1() {
        let mut dst = [0, 0, 0] as [c_int; 3];
        let expected = [0x01010101, 0x01010101, 0x01010101] as [c_int; 3];

        unsafe {
            // Set each byte to 1, which results in each int being 0x01010101
            memset(dst.as_mut_ptr() as *mut c_void, 1 as c_int, 3 * size_of::<c_int>());
            assert_eq!(dst, expected);
        }
    }

    #[test]
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

    #[test]
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