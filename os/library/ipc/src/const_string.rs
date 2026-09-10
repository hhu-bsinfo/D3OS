use core::fmt::Display;
use core::slice::SliceIndex;
use core::ops::{Index, IndexMut};

#[derive(Debug, Clone, Copy)]
/// nul-terminated, const sized string
pub struct ConstString<const T: usize> {
    buffer: [u8; T],
    len: usize,
}

impl <const T: usize> ConstString<T> {
    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buffer[..self.len]).unwrap_or("UTF8-ERROR")
    }
    pub fn len(&self) -> usize {
        self.len
    }
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    pub fn set(&mut self, s: &str) {
        let l = core::cmp::min(T, s.len());
        self.buffer[..l].copy_from_slice(&s.as_bytes()[..l]);
        self.len = l;
    }
    pub fn clear(&mut self) {
        self.buffer.fill(0);
        self.len = 0;
    }
    pub fn append(&mut self, s: &str) {
        let idx = self.len();
        let space_left = T - idx;
        let l = core::cmp::min(space_left, s.len());
        self.buffer[..l].copy_from_slice(&s.as_bytes()[..l]);
        self.len = l;
    }
}

impl ConstString<0> {
    pub fn empty() -> Self {
        Self { buffer: [0; 0], len: 0 }
    }
}

impl <const T: usize> ConstString<T> {
    pub fn new() -> Self {
        Self { buffer: [0; T], len: 0 }
    }
}

impl<const T: usize> Default for ConstString<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl <const T: usize> From<[u8; T]> for ConstString<T> {
    fn from(value: [u8; T]) -> Self {
        Self { buffer: value, len: value.len() }
    }
}

impl <const T: usize> From<&str> for ConstString<T> {
    fn from(value: &str) -> Self {
        let mut buffer = [0_u8; T];
        let l = core::cmp::min(value.len(), T);
        buffer[..l].copy_from_slice(&value.as_bytes()[..l]);
        Self { buffer, len: value.len() }
    }
}

impl <const T: usize> From<&[u8]> for ConstString<T> {
    fn from(value: &[u8]) -> Self {
        let mut buffer = [0_u8; T];
        let l = core::cmp::min(value.len(), T);
        buffer[..l].copy_from_slice(&value[..l]);
        Self { buffer, len: value.len() }
    }
}

impl <const T: usize, I: SliceIndex<[u8]>> Index<I> for ConstString<T> {
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        Index::index(&self.buffer, index)
    }
}

impl <const T: usize, I: SliceIndex<[u8]>> IndexMut<I> for ConstString<T> {
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(&mut self.buffer, index)
    }
}

impl <const T: usize> Display for ConstString<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}