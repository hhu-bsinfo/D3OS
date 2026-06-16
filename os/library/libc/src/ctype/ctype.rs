use core::ffi::c_int;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn isspace(c: c_int) -> c_int {
    if (c >= 9 && c <= 13) || c == 32 {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn toupper(c: c_int) -> c_int {
    if c >= ('a' as c_int) && c <= ('z' as c_int) {
        c - 32
    } else {
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_isspace() {
        unsafe {
            assert_eq!(isspace(32), 1); // space
            assert_eq!(isspace(9), 1);  // tab
            assert_eq!(isspace(10), 1); // newline
            assert_eq!(isspace(65), 0); // 'A'
        }
    }

    #[test]
    fn test_toupper() {
        unsafe {
            assert_eq!(toupper('a' as c_int), 'A' as c_int);
            assert_eq!(toupper('z' as c_int), 'Z' as c_int);
            assert_eq!(toupper('A' as c_int), 'A' as c_int);
            assert_eq!(toupper('1' as c_int), '1' as c_int);
        }
    }
    }