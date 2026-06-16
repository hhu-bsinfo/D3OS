use core::ffi::{c_char, c_double, c_float, CStr};
use core::str::FromStr;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn atof(str: *const c_char) -> c_double {
    unsafe {
        strtod(str, core::ptr::null_mut())
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtof(str: *const c_char, endptr: *mut *mut c_char) -> c_float {
    // Closure to determine if a character is invalid for atoi.
    let invalid_char = |c : char| {
        !c.is_digit(10) && c != '+' && c != '-' && c != '.' && c != 'e' && c != 'E'
    };

    unsafe {
        let num_str = CStr::from_ptr(str)
            // Convert C string to Rust string (default to "" if conversion fails)
            .to_str()
            .unwrap_or("")
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

        // Parse the number, defaulting to 0 if parsing fails
        c_float::from_str(num_str).unwrap_or(0.0)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn strtod(str: *const c_char, endptr: *mut *mut c_char) -> c_double {
    // Closure to determine if a character is invalid for atoi.
    let invalid_char = |c : char| {
        !c.is_digit(10) && c != '+' && c != '-' && c != '.' && c != 'e' && c != 'E'
    };

    unsafe {
        let num_str = CStr::from_ptr(str)
            // Convert C string to Rust string (default to "" if conversion fails)
            .to_str()
            .unwrap_or("")
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

        // Parse the number, defaulting to 0 if parsing fails
        c_double::from_str(num_str).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use alloc::ffi::CString;
    use super::*;

    #[test]
    fn test_atof() {
        unsafe {
            let result = atof(CString::new("123.45").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 123.45);

            let result = atof(CString::new("-123.45").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -123.45);
            
            let result = atof(CString::new("0").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 0.0);
        }
    }

    #[test]
    fn test_atof_space() {
        unsafe {
            let result = atof(CString::new("      -123.45").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -123.45);


            let result = atof(CString::new("-123.45       ").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -123.45);

            let result = atof(CString::new("   -123.45   ").unwrap().as_c_str().as_ptr());
            assert_eq!(result, -123.45);
        }
    }
    
    #[test]
    fn test_atof_empty() {
        unsafe {
            let result = atof(CString::new("").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 0.0);
        }
    }

    #[test]
    fn test_atof_negative_wrong_format() {
        unsafe {
            let result = atof(CString::new("--123").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 0.0);
        }
    }

    #[test]
    fn test_text() {
        unsafe {
            let result = atof(CString::new("Hello!").unwrap().as_c_str().as_ptr());
            assert_eq!(result, 0.0);
        }
    }
}