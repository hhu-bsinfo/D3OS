use core::fmt::{Display, Write};
use core::slice::SliceIndex;
use core::ops::{Index, IndexMut};

#[derive(Debug, Clone, Copy)]
/// nul-terminated, const sized string
pub struct ConstString<const S: usize> {
    buffer: [u8; S],
    /// length of the string, not the size of the buffer
    len: usize,
}

impl <const S: usize> ConstString<S> {
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
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// the amount of 'empty space' left in this buffer
    pub fn space_left(&self) -> usize {
        S - self.len()
    }
    
    /// set this string to `s`
    pub fn set(&mut self, s: &str) {
        let l = core::cmp::min(S, s.len());
        self.buffer[..l].copy_from_slice(&s.as_bytes()[..l]);
        self.len = l;
    }
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.len = 0;
    }
    /// append `s` to the end of this string
    pub fn append(&mut self, s: &str) {
        let idx = self.len();
        let space_left = S - idx;
        let l = core::cmp::min(space_left, s.len());
        self.buffer[..l].copy_from_slice(&s.as_bytes()[..l]);
        self.len = l;
    }
    pub fn copy_from<const T: usize>(&mut self, other: ConstString<T>) {
        let l = core::cmp::min(S, other.len());
        self.buffer[..l].copy_from_slice(&other[..l]);
        self.len = l;
    }
}

impl ConstString<0> {
    pub fn empty() -> Self {
        Self { buffer: [0; 0], len: 0 }
    }
}

impl <const S: usize> ConstString<S> {
    pub fn new() -> Self {
        Self { buffer: [0; S], len: 0 }
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
        let l = core::cmp::min(value.len(), S);
        buffer[..l].copy_from_slice(&value.as_bytes()[..l]);
        Self { buffer, len: value.len() }
    }
}

impl <const S: usize> From<&[u8]> for ConstString<S> {
    fn from(value: &[u8]) -> Self {
        let mut buffer = [0_u8; S];
        let l = core::cmp::min(value.len(), S);
        buffer[..l].copy_from_slice(&value[..l]);
        Self { buffer, len: value.len() }
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
