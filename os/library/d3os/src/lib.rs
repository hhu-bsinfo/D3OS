#![no_std]

pub mod const_string;


pub trait CopySizeChecked<T> {
    /// performs a copy_from_slice / memcpy with automatic size checks, i.e. avoids panics on different slice lengths \
    /// returns the copied length
    fn copy_from_slice_sizechecked(&mut self, src: &[T]) -> usize;
}

impl<T: Copy> CopySizeChecked<T> for &mut [T] {
    fn copy_from_slice_sizechecked(&mut self, src: &[T]) -> usize {
        let l = core::cmp::min(self.len(), src.len());
        self[..l].copy_from_slice(&src[..l]);
        l
    }
}

impl<T: Copy> CopySizeChecked<T> for [T] {
    fn copy_from_slice_sizechecked(&mut self, src: &[T]) -> usize {
        let l = core::cmp::min(self.len(), src.len());
        // do not copy when either length is zero
        if l == 0 { return 0; }
        self[..l].copy_from_slice(&src[..l]);
        l
    }
}