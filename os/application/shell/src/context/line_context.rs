use core::cmp::min;

use alloc::string::String;

#[derive(Debug, Clone, Default)]
pub struct LineContext {
    line: String,
    dirty_index: usize,
}

impl LineContext {
    pub fn new() -> Self {
        LineContext::default()
    }

    pub fn reset(&mut self) {
        *self = LineContext::default();
    }

    pub fn mark_clean(&mut self) {
        self.dirty_index = self.line.len();
    }

    pub fn mark_dirty_at(&mut self, index: usize) {
        self.dirty_index = min(self.dirty_index, index);
    }

    pub fn get(&self) -> &String {
        &self.line
    }

    pub fn get_dirty_part(&self) -> &str {
        &self.line[self.dirty_index..]
    }

    pub fn get_dirty_index(&self) -> usize {
        self.dirty_index
    }

    pub fn len(&self) -> usize {
        self.line.len()
    }

    pub fn push(&mut self, ch: char) {
        self.line.push(ch);
    }

    pub fn push_str(&mut self, string: &str) {
        self.line.push_str(string);
    }

    pub fn pop(&mut self) -> Option<char> {
        let ch = self.line.pop();
        if ch.is_some() {
            self.mark_dirty_at(self.line.len());
        }
        ch
    }

    pub fn insert(&mut self, index: usize, ch: char) {
        self.line.insert(index, ch);
        self.mark_dirty_at(index);
    }

    pub fn remove(&mut self, index: usize) {
        self.line.remove(index);
        self.mark_dirty_at(index);
    }
}
