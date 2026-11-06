/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: canonical                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Per-session canonical (line-edited) input buffer.               ║
   ║                                                                         ║
   ║ Canonical local echo is emulator-owned: as the user edits a line, the   ║
   ║ editor produces ANSI echo bytes that are fed back into that session's   ║
   ║ semantic terminal (so the edit survives tab switches), and on submit it ║
   ║ yields the completed line for delivery to the app's stdin.              ║
   ║                                                                         ║
   ║ Each session owns its own `CanonicalEditor`, so an in-progress line in  ║
   ║ one tab never leaks into another.                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::{format, string::String, vec::Vec};

use crate::terminal::model::char_columns;

/// Upper bound on the edited line, in bytes, so a single line can never
/// outgrow the `in` pipe. Checked against the encoded length of the inserted
/// character so the buffer is never cut mid-codepoint.
const BUFFER_SIZE: usize = 256;

/// A single canonical-mode edit request derived from a decoded key.
pub enum CanonicalAction {
    Insert(char),
    Backspace,
    Delete,
    Left,
    Right,
    Home,
    End,
    Submit,
}

/// The result of applying a `CanonicalAction`: echo bytes to render into the
/// session's semantic terminal, and, on submit, the completed line bytes.
pub struct CanonicalEffect {
    pub echo: String,
    pub submit: Option<Vec<u8>>,
}

impl CanonicalEffect {
    fn none() -> Self {
        Self {
            echo: String::new(),
            submit: None,
        }
    }

    fn echo(echo: String) -> Self {
        Self { echo, submit: None }
    }
}

pub struct CanonicalEditor {
    /// Cursor position as a *character* index into `buffer`. Byte offsets are
    /// derived on demand; storing bytes here and stepping them per keystroke
    /// lands inside multi-byte characters and panics on the next slice.
    cursor_pos: usize,
    buffer: String,
}

impl CanonicalEditor {
    pub const fn new() -> Self {
        Self {
            cursor_pos: 0,
            buffer: String::new(),
        }
    }

    pub fn apply(&mut self, action: CanonicalAction) -> CanonicalEffect {
        match action {
            CanonicalAction::Left => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    let columns = self.columns_at(self.cursor_pos);
                    CanonicalEffect::echo(move_left(columns))
                } else {
                    CanonicalEffect::none()
                }
            }
            CanonicalAction::Right => {
                if self.cursor_pos < self.char_count() {
                    let columns = self.columns_at(self.cursor_pos);
                    self.cursor_pos += 1;
                    CanonicalEffect::echo(move_right(columns))
                } else {
                    CanonicalEffect::none()
                }
            }
            CanonicalAction::Home => {
                let columns = display_columns(&self.buffer[..self.byte_offset(self.cursor_pos)]);
                self.cursor_pos = 0;
                CanonicalEffect::echo(move_left(columns))
            }
            CanonicalAction::End => {
                let columns = display_columns(&self.buffer[self.byte_offset(self.cursor_pos)..]);
                self.cursor_pos = self.char_count();
                CanonicalEffect::echo(move_right(columns))
            }
            CanonicalAction::Backspace => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                    let offset = self.byte_offset(self.cursor_pos);
                    let removed = self.buffer.remove(offset);
                    let echo = format!(
                        "{}{}",
                        move_left(char_columns(removed) as usize),
                        self.redraw_tail()
                    );
                    CanonicalEffect::echo(echo)
                } else {
                    CanonicalEffect::none()
                }
            }
            CanonicalAction::Delete => {
                if self.cursor_pos < self.char_count() {
                    let offset = self.byte_offset(self.cursor_pos);
                    self.buffer.remove(offset);
                    CanonicalEffect::echo(self.redraw_tail())
                } else {
                    CanonicalEffect::none()
                }
            }
            CanonicalAction::Insert(ch) => {
                if self.buffer.len() + ch.len_utf8() <= BUFFER_SIZE {
                    let offset = self.byte_offset(self.cursor_pos);
                    self.buffer.insert(offset, ch);
                    self.cursor_pos += 1;
                    CanonicalEffect::echo(format!("{}{}", ch, self.redraw_tail()))
                } else {
                    CanonicalEffect::none()
                }
            }
            CanonicalAction::Submit => {
                let columns = display_columns(&self.buffer[self.byte_offset(self.cursor_pos)..]);
                let echo = format!("{}\n", move_right(columns));
                let line = self.buffer.clone().into_bytes();
                self.buffer.clear();
                self.cursor_pos = 0;
                CanonicalEffect {
                    echo,
                    submit: Some(line),
                }
            }
        }
    }

    /// Echo needed to redraw the buffer tail after the cursor when characters
    /// are inserted or removed mid-line, leaving the cursor in place. The
    /// leading erase also clears whatever the previous, longer tail left
    /// behind, so callers never have to blank cells themselves.
    ///
    /// Column arithmetic assumes the edited line occupies a single screen row;
    /// a line long enough to wrap still echoes correctly up to the wrap point.
    fn redraw_tail(&self) -> String {
        let tail = &self.buffer[self.byte_offset(self.cursor_pos)..];
        format!("\x1b[0K{}{}", tail, move_left(display_columns(tail)))
    }

    fn char_count(&self) -> usize {
        self.buffer.chars().count()
    }

    /// Byte offset of a character index, clamped to the end of the buffer.
    fn byte_offset(&self, char_index: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_index)
            .map_or(self.buffer.len(), |(offset, _)| offset)
    }

    /// Column span of the character at a character index.
    fn columns_at(&self, char_index: usize) -> usize {
        self.buffer
            .chars()
            .nth(char_index)
            .map_or(0, |ch| char_columns(ch) as usize)
    }
}

/// Columns the string occupies once rendered, which is not its byte length and
/// not its character count once wide glyphs are involved.
fn display_columns(text: &str) -> usize {
    text.chars().map(|ch| char_columns(ch) as usize).sum()
}

/// `CUB`, or nothing at all for a zero-column move: the model reads a `0`
/// parameter as "move by one".
fn move_left(columns: usize) -> String {
    if columns == 0 {
        String::new()
    } else {
        format!("\x1b[{}D", columns)
    }
}

/// `CUF`, with the same zero-column caveat as [`move_left`].
fn move_right(columns: usize) -> String {
    if columns == 0 {
        String::new()
    } else {
        format!("\x1b[{}C", columns)
    }
}
