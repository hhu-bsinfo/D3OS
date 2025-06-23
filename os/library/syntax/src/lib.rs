#![no_std]

#[cfg(not(feature = "alloc"))]
compile_error!("The 'alloc' feature must be enabled.");

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod clike;
pub mod located;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
