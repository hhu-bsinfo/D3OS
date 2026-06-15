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
use core::slice::from_raw_parts_mut;
use core::ptr::addr_of;

type Comparator = unsafe extern "C" fn(*const c_void, *const c_void) -> c_int;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn qsort(base: *const c_void, count: c_size_t, size: c_size_t, comp: Comparator) {
    if base == ptr::null() || count == 0 || size == 0 {
        return;
    }

    unsafe {
        let base = from_raw_parts_mut(base as *mut u8, count * size);
        bubble_sort(base, count, size, comp);
    }
}
fn bubble_sort(base: &mut [u8], count: c_size_t, size: c_size_t, comp: Comparator) -> () {
    assert!(size > 0, "bubble_sort: Element size must be greater than zero");
    assert_eq!(base.len(), count * size, "bubble_sort: Base length must match (count * size)");

    if count == 0 {
        return;
    }

    let mut swapped = true;
    while swapped {
        swapped = false;

        for i in 0..count - 1 {
            let current = addr_of!(base[i * size]) as *const c_void;
            let next = addr_of!(base[(i + 1) * size]) as *const c_void;

            if unsafe { comp(current, next) } > 0 {
                swap(base, i, i + 1, size);
                swapped = true;
            }
        }
    }
}

fn swap(base: &mut [u8], first_element: c_size_t, second_element: c_size_t, size: c_size_t) {
    // Cut into 2 mut slices, left = first element, right = second element
    let (first, second) = base.split_at_mut(second_element * size);
    first[first_element * size..(first_element + 1) * size].swap_with_slice(&mut second[0..size]);
}

#[cfg(test)]
mod tests {
    use core::ffi::c_char;
    use crate::stdlib::{comp_char, comp_int, comp_struct, TestStruct};
    use super::*;

    #[test]
    fn test_qsort_int() {
        let array = [10, 9, 8, 7, 6, 5, 4, 3, 2, 1] as [c_int; 10];
        let count = array.len() as c_size_t;
        let size = size_of::<c_int>() as c_size_t;

        unsafe {
            qsort(array.as_ptr() as *const c_void, count, size, comp_int);
            assert_eq!(array, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        }
    }

    #[test]
    fn test_qsort_empty_array() {
        let array = [] as [c_int; 0];
        let count = array.len() as c_size_t;
        let size = size_of::<c_int>() as c_size_t;

        unsafe {
            qsort(array.as_ptr() as *const c_void, count, size, comp_int);
            assert_eq!(array, []);
        }
    }

    #[test]
    fn test_qsort_with_chars() {
        let array = [
            'j' as c_char,
            'i' as c_char,
            'h' as c_char,
            'g' as c_char,
            'f' as c_char,
            'e' as c_char,
            'd' as c_char,
            'c' as c_char,
            'b' as c_char,
            'a' as c_char
        ];

        let expected = [
            'a' as c_char,
            'b' as c_char,
            'c' as c_char,
            'd' as c_char,
            'e' as c_char,
            'f' as c_char,
            'g' as c_char,
            'h' as c_char,
            'i' as c_char,
            'j' as c_char
        ];

        let count = array.len() as c_size_t;
        let size = size_of::<c_char>() as c_size_t;

        unsafe {
            qsort(array.as_ptr() as *const c_void, count, size, comp_char);
            assert_eq!(array, expected);
        }
    }

    #[test]
    fn test_qsort_with_structs() {
        let array = [
            TestStruct { value: 3, test_char: 'c' as c_char },
            TestStruct { value: 2 , test_char: 'b' as c_char},
            TestStruct { value: 1, test_char: 'a' as c_char }
        ];

        let expected = [
            TestStruct { value: 1, test_char: 'a' as c_char },
            TestStruct { value: 2, test_char: 'b' as c_char },
            TestStruct { value: 3, test_char: 'c' as c_char }
        ];

        let count = array.len() as c_size_t;
        let size = size_of::<TestStruct>() as c_size_t;

        unsafe {
            qsort(array.as_ptr() as *const c_void, count, size, comp_struct);
            assert_eq!(array, expected);
        }
    }
    #[test]
    fn test_qsort_null_pointer() {
        unsafe {
            let array = ptr::null();
            let count = 0 as c_size_t;
            let size = size_of::<c_int>() as c_size_t;

            qsort(array, count, size, comp_int);
            assert_eq!(array, ptr::null());
        }
    }
}