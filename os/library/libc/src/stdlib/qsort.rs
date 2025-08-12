/*
 * The C standard library is based on a bachelor's thesis, written by Gökhan Cöpcü.
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

use core::ffi::{c_int, c_size_t, c_void};
use core::slice::from_raw_parts_mut;
use core::ptr::addr_of;

#[unsafe(no_mangle)]
pub extern "C" fn qsort(
    base: *const c_void,
    num_element: c_size_t,
    size: c_size_t,
    comp: extern "C" fn(*const c_void, *const c_void) -> c_int
) {
    // Check for null pointers and correct size of array or elements
    if base.is_null() || num_element <= 0 || size <= 0 {
        return;
    }

    unsafe {
        let base = from_raw_parts_mut(base as *mut u8, num_element * size);
        bubble_sort(base, num_element, size, comp);
    }
}
fn bubble_sort(
    base: &mut [u8],
    num_element: c_size_t,
    size: c_size_t,
    comp: extern "C" fn(*const c_void, *const c_void) -> c_int
) -> () {
    let mut swapped = true;

    while swapped {
        swapped = false;

        for i in 0..num_element - 1 {
            let current = addr_of!(base[i * size]) as *const c_void;
            let next = addr_of!(base[(i + 1) * size]) as *const c_void;

            if comp(current, next) > 0 {
                swap(base, i, i + 1, size);
                swapped = true;
            }
        }
    }
}

fn swap(base: &mut [u8], first_element: c_size_t, second_element: c_size_t, size: c_size_t) {
    // Cut into 2 mut slices, left = first element, right = second element
    let (first, second) = base.split_at_mut(second_element * size);
    first[first_element * size.. (first_element + 1) * size].swap_with_slice(&mut second[0.. size]);
}

/*mod tests {
    use super::*;
    #[test_case]
    fn test_qsort() {
        extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const c_int;
            let b = b as *const c_int;
            unsafe {
                if *a > *b {
                    1
                } else if *a < *b {
                    -1
                } else {
                    0
                }
            }
        }

        let array = [10, 9, 8, 7, 6, 5, 4, 3, 2, 1] as [c_int; 10];
        let num_element = array.len() as c_size_t;
        let size = size_of::<c_int>();
        qsort(array.as_ptr() as *const c_void,
              num_element,
              size as c_size_t,
              comp as extern "C" fn(*const c_void, *const c_void) -> c_int);
        assert_eq!(array, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    }

    #[test_case]
    fn test_qsort_empty_array() {
        extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const c_int;
            let b = b as *const c_int;
            unsafe {
                if *a > *b {
                    1
                } else if *a < *b {
                    -1
                } else {
                    0
                }
            }
        }

        let array = [] as [c_int; 0];
        let num_element = array.len() as c_size_t;
        let size = core::mem::size_of::<c_int>();
        qsort(array.as_ptr() as *const c_void,
              num_element,
              size as c_size_t,
              comp as extern "C" fn(*const c_void, *const c_void) -> c_int);
        assert_eq!(array, []);
    }

    #[test_case]
    fn test_qsort_with_chars() {
        extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const c_char;
            let b = b as *const c_char;
            unsafe {
                if *a > *b {
                    1
                } else if *a < *b {
                    -1
                } else {
                    0
                }
            }
        }
        let array = ['j' as c_char, 'i' as c_char, 'h' as c_char, 'g' as c_char, 'f' as c_char, 'e' as c_char, 'd' as c_char, 'c' as c_char, 'b' as c_char, 'a' as c_char] as [c_char; 10];
        let num_element = array.len() as c_size_t;
        let size = core::mem::size_of::<c_char>();
        qsort(array.as_ptr() as *const c_void,
              num_element,
              size as c_size_t,
              comp as extern "C" fn(*const c_void, *const c_void) -> c_int);
        assert_eq!(array, ['a' as c_char, 'b' as c_char, 'c' as c_char, 'd' as c_char, 'e' as c_char, 'f' as c_char, 'g' as c_char, 'h' as c_char, 'i' as c_char, 'j' as c_char]);
    }

    #[test_case]
    fn test_qsort_with_structs() {
        #[repr(C)]
        struct MyStruct {
            value: c_int
        }
        extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const MyStruct;
            let b = b as *const MyStruct;
            unsafe {
                if (*a).value > (*b).value {
                    1
                } else if (*a).value < (*b).value {
                    -1
                } else {
                    0
                }
            }
        }
        let array = [MyStruct { value: 3 },
            MyStruct { value: 2 },
            MyStruct { value: 1 }] as [MyStruct; 3];
        let num_element = array.len() as c_size_t;
        let size = core::mem::size_of::<MyStruct>();
        qsort(array.as_ptr() as *const c_void,
              num_element,
              size as c_size_t,
              comp as extern "C" fn(*const c_void, *const c_void) -> c_int);
        for i in 0..array.len() - 1 {
            assert!(array[i].value <= array[i + 1].value);
        }
    }
    #[test_case]
    fn test_qsort_with_structs2() {
        #[repr(C)]
        struct MyStruct {
            value: c_int,
            test_char: c_char
        }
        extern "C" fn comp(a: *const c_void, b: *const c_void) -> c_int {
            let a = a as *const MyStruct;
            let b = b as *const MyStruct;
            unsafe {
                if (*a).value > (*b).value {
                    1
                } else if (*a).value < (*b).value {
                    -1
                } else {
                    0
                }
            }
        }
        let array = [MyStruct { value: 3, test_char: 'c' as c_char },
            MyStruct { value: 2 , test_char: 'b' as c_char},
            MyStruct { value: 1, test_char: 'a' as c_char }] as [MyStruct; 3];
        let num_element = array.len() as c_size_t;
        let size = core::mem::size_of::<MyStruct>();
        qsort(array.as_ptr() as *const c_void,
              num_element,
              size as c_size_t,
              comp as extern "C" fn(*const c_void, *const c_void) -> c_int);
        for i in 0..array.len() - 1 {
            assert!(array[i].value <= array[i + 1].value);
        }
    }
}*/