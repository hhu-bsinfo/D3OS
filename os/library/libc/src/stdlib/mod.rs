use core::cmp::Ordering;
use core::ffi::{c_char, c_int, c_void};

pub mod abort;
pub mod bsearch;
pub mod qsort;
pub mod strtol;

/*
 * Comparator functions for `bsearch` and `qsort` tests.
 */

unsafe extern "C" fn comp_int(a: *const c_void, b: *const c_void) -> c_int {
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

unsafe extern "C" fn comp_char(a: *const c_void, b: *const c_void) -> c_int {
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

#[repr(C)]
#[derive(Debug, PartialEq)]
struct TestStruct {
    value: c_int,
    test_char: c_char
}

unsafe extern "C" fn comp_struct(a: *const c_void, b: *const c_void) -> c_int {
    let a = a as *const TestStruct;
    let b = b as *const TestStruct;

    unsafe {
        match (*a).value.cmp(&((*b).value)) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1
        }
    }
}