use core::cmp::min;

use alloc::string::String;

#[derive(Debug, Clone, Default)]
pub struct LineContext {
    line: String,
    dirty_index: usize,
    cursor_position: usize,
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

    /// Character index of the dirty byte offset (for column-oriented writers).
    pub fn get_dirty_char_index(&self) -> usize {
        self.line[..self.dirty_index].chars().count()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty_index < self.line.len()
    }

    pub fn len(&self) -> usize {
        self.line.len()
    }

    /// Character length of the line (cursor end / movement).
    pub fn char_len(&self) -> usize {
        self.line.chars().count()
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

    /// Insert `ch` at a *character* index.
    pub fn insert(&mut self, char_index: usize, ch: char) {
        let byte_index = self.byte_offset(char_index);
        self.line.insert(byte_index, ch);
        self.mark_dirty_at(byte_index);
    }

    /// Remove the character at a *character* index.
    pub fn remove(&mut self, char_index: usize) {
        let byte_index = self.byte_offset(char_index);
        self.line.remove(byte_index);
        self.mark_dirty_at(byte_index);
    }

    pub fn get_cursor_pos(&self) -> usize {
        self.cursor_position
    }

    pub fn set_cursor_pos(&mut self, pos: usize) {
        self.cursor_position = pos;
    }

    pub fn move_cursor_left(&mut self, step: usize) {
        self.cursor_position -= step;
    }

    pub fn move_cursor_right(&mut self, step: usize) {
        self.cursor_position += step;
    }

    pub fn is_cursor_at_start(&self) -> bool {
        self.cursor_position == 0
    }

    pub fn is_cursor_at_end(&self) -> bool {
        self.cursor_position >= self.char_len()
    }

    fn byte_offset(&self, char_index: usize) -> usize {
        self.line
            .char_indices()
            .nth(char_index)
            .map_or(self.line.len(), |(offset, _)| offset)
    }
}
