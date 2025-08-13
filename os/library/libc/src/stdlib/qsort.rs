/*
 * The C standard library is based on a bachelor's thesis, written by Gökhan Cöpcü.
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::{c_int, c_size_t, c_void};
use core::slice::from_raw_parts_mut;
use core::ptr::addr_of;

type Comparator = unsafe extern "C" fn(*const c_void, *const c_void) -> c_int;

#[unsafe(no_mangle)]
pub extern "C" fn qsort(base: *const c_void, num_element: c_size_t, size: c_size_t, comp: Comparator) {
    // Check for correct size of array or elements
    assert!(size > 0, "qsort: Element size must be greater than zero");

    unsafe {
        let base = from_raw_parts_mut(base as *mut u8, num_element * size);
        bubble_sort(base, num_element, size, comp);
    }
}
fn bubble_sort(base: &mut [u8], num_element: c_size_t, size: c_size_t, comp: Comparator) -> () {
    assert!(size > 0, "bubble_sort: Element size must be greater than zero");
    assert_eq!(base.len(), num_element * size, "bubble_sort: Base length must match num_element * size");

    if num_element == 0 {
        return;
    }

    let mut swapped = true;
    while swapped {
        swapped = false;

        for i in 0..num_element - 1 {
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
    use core::cmp::Ordering;
    use core::ffi::c_char;
    use core::ptr;
    use super::*;

    #[test_case]
    fn test_qsort_int() {
        unsafe extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const c_int;
            let b = b as *const c_int;

            unsafe {
                match (*a).cmp(&*b) {
                    Ordering::Less => -1,
                    Ordering::Equal => 0,
                    Ordering::Greater => 1
                }
            }
        }

        let array = [10, 9, 8, 7, 6, 5, 4, 3, 2, 1] as [c_int; 10];
        let num_element = array.len() as c_size_t;
        let size = size_of::<c_int>() as c_size_t;

        qsort(array.as_ptr() as *const c_void, num_element, size, comp);
        assert_eq!(array, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test_case]
    fn test_qsort_empty_array() {
        unsafe extern "C" fn comp(_a: *const c_void, _b: *const c_void) -> c_int {
            0
        }

        let array = [] as [c_int; 0];
        let num_element = array.len() as c_size_t;
        let size = size_of::<c_int>() as c_size_t;

        qsort(array.as_ptr() as *const c_void, num_element, size, comp);
        assert_eq!(array, []);
    }

    #[test_case]
    fn test_qsort_with_chars() {
        unsafe extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const c_char;
            let b = b as *const c_char;

            unsafe {
                match (*a).cmp(&*b) {
                    Ordering::Less => -1,
                    Ordering::Equal => 0,
                    Ordering::Greater => 1
                }
            }
        }

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

        let num_element = array.len() as c_size_t;
        let size = size_of::<c_char>() as c_size_t;

        qsort(array.as_ptr() as *const c_void, num_element, size, comp);
        assert_eq!(array, expected);
    }

    #[test_case]
    fn test_qsort_with_structs() {
        #[repr(C)]
        #[derive(Debug, PartialEq)]
        struct MyStruct {
            value: c_int,
            test_char: c_char
        }

        unsafe extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const MyStruct;
            let b = b as *const MyStruct;

            unsafe {
                match (*a).value.cmp(&((*b).value)) {
                    Ordering::Less => -1,
                    Ordering::Equal => 0,
                    Ordering::Greater => 1
                }
            }
        }

        let array = [
            MyStruct { value: 3, test_char: 'c' as c_char },
            MyStruct { value: 2 , test_char: 'b' as c_char},
            MyStruct { value: 1, test_char: 'a' as c_char }
        ];

        let expected = [
            MyStruct { value: 1, test_char: 'a' as c_char },
            MyStruct { value: 2, test_char: 'b' as c_char },
            MyStruct { value: 3, test_char: 'c' as c_char }
        ];

        let num_element = array.len() as c_size_t;
        let size = size_of::<MyStruct>() as c_size_t;

        qsort(array.as_ptr() as *const c_void, num_element, size, comp);
        assert_eq!(array, expected);
    }

    #[test_case]
    #[should_panic]
    fn test_qsort_null_pointer() {
        unsafe extern "C" fn comp(_a: *const c_void, _b: *const c_void) -> c_int {
            0
        }

        qsort(ptr::null(), 0, 0, comp);
    }
}