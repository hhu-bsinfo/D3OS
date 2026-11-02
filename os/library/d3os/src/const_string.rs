use core::fmt::{Display, Write};
use core::slice::SliceIndex;
use core::ops::{Index, IndexMut};
use crate::copy_from_slice_sizechecked;

#[derive(Debug, Clone, Copy)]
/// nul-terminated, const sized string
pub struct ConstString<const S: usize> {
    buffer: [u8; S],
    /// length of the string, not the size of the buffer
    len: usize,
}

impl <const S: usize> ConstString<S> {

    pub const fn new() -> Self {
        Self { buffer: [0; S], len: 0 }
    }

    /// returns this buffer as a str
    pub fn as_str(&self) -> Result<&str, core::str::Utf8Error> {
        core::str::from_utf8(&self.buffer[..self.len])
    }
    /// returns this buffer as a str; on UTF8 error, it returns 'UTF8-ERROR'
    pub fn as_str_unchecked(&self) -> &str {
        core::str::from_utf8(&self.buffer[..self.len]).unwrap_or("UTF8-ERROR")
    }
    /// length of this string, not the underlying buffer
    pub fn len(&self) -> usize {
        self.len
    }
    #[must_use]
    /// is this str empty (i.e. "")
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// the amount of 'empty space' left in this buffer
    pub fn space_left(&self) -> usize {
        S - self.len()
    }
    pub fn as_ptr(&self) -> *const u8 {
        self.buffer.as_ptr()
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.buffer[..self.len]
    }
    
    pub fn set_from_bytes(&mut self, bytes: &[u8]) {
        let new_len = copy_from_slice_sizechecked!(self.buffer, bytes);
        self.len = new_len;
    }
    /// set this string to `s`
    pub fn set(&mut self, s: &str) {
        let l = copy_from_slice_sizechecked!(self.buffer, s.as_bytes());
        self.len = l;
    }
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.len = 0;
    }
    /// append `s` to the end of this string
    pub fn append(&mut self, s: &str) {
        let idx = self.len();
        let l = copy_from_slice_sizechecked!(self.buffer[idx..], s.as_bytes());
        self.len += l;
    }
    pub fn copy_from<const T: usize>(&mut self, other: ConstString<T>) {
        let l = copy_from_slice_sizechecked!(self.buffer, other);
        self.len = l;
    }
}

impl ConstString<0> {
    pub const fn empty() -> Self {
        Self { buffer: [0; 0], len: 0 }
    }
}

impl<const S: usize> Default for ConstString<S> {
    fn default() -> Self {
        Self::new()
    }
}

impl <const S: usize> From<[u8; S]> for ConstString<S> {
    fn from(value: [u8; S]) -> Self {
        Self { buffer: value, len: value.len() }
    }
}

impl <const S: usize> From<&str> for ConstString<S> {
    fn from(value: &str) -> Self {
        let mut buffer = [0_u8; S];
        let l = copy_from_slice_sizechecked!(buffer, value.as_bytes());
        Self { buffer, len: l }
    }
}

impl <const S: usize> From<&[u8]> for ConstString<S> {
    fn from(value: &[u8]) -> Self {
        let mut buffer = [0_u8; S];
        let l = copy_from_slice_sizechecked!(buffer, value);
        Self { buffer, len: l }
    }
}

impl <const S: usize, I: SliceIndex<[u8]>> Index<I> for ConstString<S> {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        Index::index(&self.buffer, index)
    }
}

impl <const S: usize, I: SliceIndex<[u8]>> IndexMut<I> for ConstString<S> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut self.buffer, index)
    }
}

impl <const S: usize> Display for ConstString<S> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str_unchecked())
    }
}

impl <const S: usize> Write for ConstString<S> {
    /// this function will never fail; if the string is full, nothing will be written
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.append(s);
        Ok(())
    }
}

impl <const S: usize> PartialEq for ConstString<S> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str_unchecked() == other.as_str_unchecked()
    }
}

impl <const S: usize> PartialEq<str> for ConstString<S> {
    fn eq(&self, other: &str) -> bool {
        self.as_str_unchecked() == other
    }
}