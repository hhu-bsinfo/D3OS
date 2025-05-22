/*
    piece table text buffer  --  Julius Drodofsky
    implements Iterator,
    undo()
    from_str(&str)
    delete(logical_adress, false)
    // if logical_adress > n
        append
    insert(logical_adress, char)
    to_string()
    get_char(u)
*/

#![no_std]

#[cfg(not(feature = "alloc"))]
compile_error!("The 'alloc' feature must be enabled.");

#[cfg(feature = "alloc")]
extern crate alloc;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TextBufferError {
    AddressOutOfBounds,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum BufferDescr {
    File,
    Add,
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Operation {
    Insert(usize, char),
    Delete(usize, char),
}

#[derive(Debug, PartialEq, Clone, Copy)]
struct PieceDescr {
    buffer: BufferDescr,
    offset: usize,
    length: usize,
}
#[derive(Debug, PartialEq, Clone)]
pub struct TextBuffer<'s> {
    file_buffer: &'s str,
    add_buffer: String,
    piece_table: Vec<PieceDescr>,
    lenght: usize,
    pos: usize,
    history: Vec<Operation>,
    redo: Vec<Operation>,
}

impl<'s> TextBuffer<'s> {
    pub fn len(&self) -> usize {
        self.lenght
    }
    pub fn get_char(&self, logical_adress: usize) -> Option<char> {
        let (piece_table_index, piece_descr_offset) =
            self.resolve_logical_adress(logical_adress, false)?;
        let piece = self.piece_table.get(piece_table_index)?;
        match piece.buffer {
            BufferDescr::Add => self
                .add_buffer
                .chars()
                .nth(piece.offset + piece_descr_offset),
            BufferDescr::File => self
                .file_buffer
                .chars()
                .nth(piece.offset + piece_descr_offset),
        }
    }

    pub fn undo(&mut self) -> Result<(), TextBufferError> {
        let operation = match self.history.pop() {
            Some(o) => match o {
                Operation::Insert(la, _) => return self._delete(la, true),
                Operation::Delete(la, c) => return self._insert(la, c, true),
            },
            None => return Ok(()),
        };
    }

    pub fn insert(&mut self, logical_adress: usize, c: char) -> Result<(), TextBufferError> {
        self._insert(logical_adress, c, false)
    }
    fn _insert(
        &mut self,
        logical_adress: usize,
        c: char,
        redo: bool,
    ) -> Result<(), TextBufferError> {
        let (piece_table_index, piece_descr_offset) =
            match self.resolve_logical_adress(logical_adress, true) {
                Some((i, o)) => (i, o),
                None => return Err(TextBufferError::AddressOutOfBounds),
            };
        self.add_buffer.push(c);
        // Enlarge piece_table entry if possible:
        if piece_table_index > 0
            && self.piece_table[piece_table_index - 1].buffer == BufferDescr::Add
            && self.piece_table[piece_table_index - 1].offset
                + self.piece_table[piece_table_index - 1].length
                == self.add_buffer.len() - 1
        {
            self.piece_table[piece_table_index - 1].length += 1;
            self.lenght += 1;
            if redo {
                self.redo.push(Operation::Insert(logical_adress, c));
            } else {
                self.history.push(Operation::Insert(logical_adress, c));
            }
            return Ok(());
        }
        // Appen if piece_table index = n:
        if piece_table_index == self.piece_table.len() {
            self.piece_table.insert(
                piece_table_index,
                PieceDescr::new(BufferDescr::Add, self.add_buffer.len() - 1, 1),
            );
            self.lenght += 1;
            if redo {
                self.redo.push(Operation::Insert(logical_adress, c));
            } else {
                self.history.push(Operation::Insert(logical_adress, c));
            }
            return Ok(());
        }
        let piece_descr = &mut self.piece_table[piece_table_index];
        if piece_descr_offset == 0 {
            self.piece_table.insert(
                piece_table_index,
                PieceDescr::new(BufferDescr::Add, self.add_buffer.len() - 1, 1),
            );
        } else {
            let length = piece_descr.length;
            let buffer = piece_descr.buffer;
            piece_descr.length = piece_descr_offset;
            self.piece_table.insert(
                piece_table_index + 1,
                PieceDescr::new(
                    buffer,
                    self.piece_table[piece_table_index].offset + piece_descr_offset,
                    length - piece_descr_offset,
                ),
            );
            self.piece_table.insert(
                piece_table_index + 1,
                PieceDescr::new(BufferDescr::Add, self.add_buffer.len() - 1, 1),
            );
        }

        self.lenght += 1;
        if redo {
            self.redo.push(Operation::Insert(logical_adress, c));
        } else {
            self.history.push(Operation::Insert(logical_adress, c));
        }
        Ok(())
    }

    // returns (index to piecetable entry,  possition in piece_descr_span (offset<=i<length))
    fn resolve_logical_adress(
        &self,
        logical_adress: usize,
        // if logical_adress > range => return (n,0)
        append: bool,
    ) -> Option<(usize, usize)> {
        let mut piece_table_index = 0;
        let mut la_start = 0;
        while let Some(piece_descr) = self.piece_table.get(piece_table_index) {
            if logical_adress >= la_start && logical_adress < la_start + piece_descr.length {
                return Some((piece_table_index, logical_adress - la_start));
            }
            la_start += piece_descr.length;
            piece_table_index += 1;
        }
        if append {
            return Some((piece_table_index, 0));
        }
        None
    }

    pub fn delete(&mut self, logical_adress: usize) -> Result<(), TextBufferError> {
        self._delete(logical_adress, false)
    }
    fn _delete(&mut self, logical_adress: usize, redo: bool) -> Result<(), TextBufferError> {
        let (piece_table_index, piece_descr_offset) =
            match self.resolve_logical_adress(logical_adress, false) {
                Some((i, o)) => (i, o),
                None => return Err(TextBufferError::AddressOutOfBounds),
            };
        let c = match self.get_char(logical_adress) {
            Some(s) => s,
            None => return Err(TextBufferError::AddressOutOfBounds),
        };
        let piece_descr = &mut self.piece_table[piece_table_index];
        // delete at beginning
        if piece_descr_offset == 0 {
            piece_descr.set_offset(piece_descr.offset + 1);
            piece_descr.set_length(piece_descr.length - 1);
        // delete at end
        } else if piece_descr.length - 1 == piece_descr_offset {
            piece_descr.set_length(piece_descr.length - 1);
        // delete in middle
        } else {
            let length = piece_descr.length;
            piece_descr.length = piece_descr_offset;
            self.piece_table.insert(
                piece_table_index + 1,
                PieceDescr::new(
                    BufferDescr::File,
                    self.piece_table[piece_table_index].offset + piece_descr_offset + 1,
                    length - piece_descr_offset - 1,
                ),
            );
        }
        // remove "empty" piece descriptor
        if self.piece_table.get(piece_table_index).unwrap().length == 0 {
            self.piece_table.remove(piece_table_index);
        }
        self.lenght -= 1;
        if redo {
            self.redo.push(Operation::Delete(logical_adress, c));
        } else {
            self.history.push(Operation::Delete(logical_adress, c));
        }
        Ok(())
    }

    pub fn from_str(file_buffer: &'s str) -> Self {
        Self {
            file_buffer,
            add_buffer: String::new(),
            piece_table: vec![PieceDescr::new(BufferDescr::File, 0, file_buffer.len())],
            lenght: file_buffer.len(),
            pos: 0,
            history: Vec::new(),
            redo: Vec::new(),
        }
    }
    pub fn to_string(&self) -> String {
        let mut i = 0;
        let mut ret = String::new();
        while let Some(c) = self.get_char(i) {
            ret.push(c);
            i += 1;
        }
        ret
    }
}

impl Iterator for TextBuffer<'_> {
    type Item = char;
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.lenght {
            let result = self.get_char(self.pos);
            self.pos += 1;
            result
        } else {
            None
        }
    }
}

impl PieceDescr {
    fn new(buffer: BufferDescr, offset: usize, length: usize) -> Self {
        Self {
            buffer,
            offset,
            length,
        }
    }
    fn set_offset(&mut self, offset: usize) {
        self.offset = offset;
    }
    fn set_length(&mut self, length: usize) {
        self.length = length;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_string(text: &TextBuffer) -> String {
        let mut i = 0;
        let mut ret = String::new();
        while let Some(c) = text.get_char(i) {
            ret.push(c);
            i += 1;
        }
        ret
    }

    #[test]
    fn initiate_textbuffer() {
        let file_buffer = "Hello World";
        let buffer = TextBuffer::from_str(file_buffer);
        assert_eq!(buffer.file_buffer, file_buffer);
        assert_eq!(buffer.add_buffer, "");
        assert_eq!(
            buffer.piece_table,
            vec![PieceDescr {
                buffer: BufferDescr::File,
                offset: 0,
                length: 11
            }]
        );
        assert_eq!(String::from(file_buffer), generate_string(&buffer));
    }
    #[test]
    fn resolve_adress() {
        let file_buffer = "Hello World";
        let mut buffer = TextBuffer::from_str(file_buffer);
        //      buffer.delete(4);
        buffer.piece_table.remove(0);
        buffer.piece_table.push(PieceDescr {
            buffer: BufferDescr::File,
            offset: 0,
            length: 4,
        });
        buffer.piece_table.push(PieceDescr {
            buffer: BufferDescr::File,
            offset: 5,
            length: 6,
        });
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 4
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 5,
                    length: 6
                }
            ]
        );
        assert_eq!(buffer.resolve_logical_adress(0, false), Some((0, 0)));
        assert_eq!(buffer.resolve_logical_adress(1, false), Some((0, 1)));
        assert_eq!(buffer.resolve_logical_adress(2, false), Some((0, 2)));
        assert_eq!(buffer.resolve_logical_adress(3, false), Some((0, 3)));
        assert_eq!(buffer.resolve_logical_adress(4, false), Some((1, 0)));
        assert_eq!(buffer.resolve_logical_adress(5, false), Some((1, 1)));
        assert_eq!(buffer.resolve_logical_adress(6, false), Some((1, 2)));
        assert_eq!(buffer.resolve_logical_adress(7, false), Some((1, 3)));
        assert_eq!(buffer.resolve_logical_adress(8, false), Some((1, 4)));
        assert_eq!(buffer.resolve_logical_adress(9, false), Some((1, 5)));
    }

    #[test]
    fn single_delete_at_beginning() {
        let file_buffer = "ab";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(0);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![PieceDescr {
                buffer: BufferDescr::File,
                offset: 1,
                length: 1
            }]
        );
        assert_eq!(String::from("b"), generate_string(&buffer));
    }

    #[test]
    fn single_delete_at_end() {
        let file_buffer = "ab";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(1);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![PieceDescr {
                buffer: BufferDescr::File,
                offset: 0,
                length: 1
            }]
        );
        assert_eq!(String::from("a"), generate_string(&buffer));
    }

    #[test]
    fn single_delete() {
        let file_buffer = "abcde";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(2);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 2
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 3,
                    length: 2
                }
            ]
        );
        assert_eq!(String::from("abde"), generate_string(&buffer));
    }
    #[test]
    fn muliple_deletion1() {
        let file_buffer = "abcdef";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(1);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 2,
                    length: 4
                }
            ]
        );
        assert_eq!(String::from("acdef"), generate_string(&buffer));
        let res = buffer.delete(3);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 2,
                    length: 2
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 5,
                    length: 1
                }
            ]
        );
        assert_eq!(String::from("acdf"), generate_string(&buffer));
    }

    #[test]
    fn muliple_deletion2() {
        let file_buffer = "abcdef";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(2);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 2
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 3,
                    length: 3
                }
            ]
        );
        assert_eq!(String::from("abdef"), generate_string(&buffer));
        let res = buffer.delete(2);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 2
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 4,
                    length: 2
                }
            ]
        );
        assert_eq!(String::from("abef"), generate_string(&buffer));
        let res = buffer.delete(3);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 2
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 4,
                    length: 1
                }
            ]
        );
        assert_eq!(String::from("abe"), generate_string(&buffer));
    }
    #[test]
    fn multiple_deletion3() {
        let file_buffer = "abcd";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(1);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 2,
                    length: 2
                }
            ]
        );
        assert_eq!(String::from("acd"), generate_string(&buffer));
        let res = buffer.delete(0);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![PieceDescr {
                buffer: BufferDescr::File,
                offset: 2,
                length: 2
            }]
        );
        assert_eq!(String::from("cd"), generate_string(&buffer));
        let res = buffer.delete(1);
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![PieceDescr {
                buffer: BufferDescr::File,
                offset: 2,
                length: 1
            }]
        );
        assert_eq!(String::from("c"), generate_string(&buffer));
        let res = buffer.delete(0);
        assert!(res.is_ok());
        assert_eq!(buffer.piece_table, vec![]);
        assert_eq!(String::from(""), generate_string(&buffer));
    }
    #[test]
    fn delete_address_out_of_bounds() {
        let file_buffer = "abcd";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let ret = buffer.delete(4);
        assert_eq!(ret, Err(TextBufferError::AddressOutOfBounds));
    }
    #[test]
    fn access_address_out_of_bounds() {
        let file_buffer = "ABCD";
        let buffer = TextBuffer::from_str(file_buffer);
        let ret = buffer.resolve_logical_adress(4, false);
        assert_eq!(ret, None);
    }
    #[test]
    fn access_address_out_of_bounds_append() {
        let file_buffer = "ABCD";
        let buffer = TextBuffer::from_str(file_buffer);
        let ret = buffer.resolve_logical_adress(4, true);
        assert_eq!(ret, Some((1, 0)));
    }
    #[test]
    fn single_insertion_at_beginning() {
        let file_buffer = "B";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(0, 'A');
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                }
            ]
        );
        assert_eq!(String::from("AB"), generate_string(&buffer));
    }
    #[test]
    fn single_insertion_in_middle() {
        let file_buffer = "AC";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(1, 'B');
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 1,
                    length: 1
                },
            ]
        );
        assert_eq!(String::from("ABC"), generate_string(&buffer));
    }

    #[test]
    fn multiple_insertion_in_middle() {
        let file_buffer = "AD";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(1, 'B');
        assert!(res.is_ok());
        assert_eq!(String::from("ABD"), generate_string(&buffer));
        let res = buffer.insert(2, 'C');
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 2
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 1,
                    length: 1
                },
            ]
        );
        assert_eq!(String::from("ABCD"), generate_string(&buffer));
    }
    // only from file
    #[test]
    fn get_i() {
        let file_buffer = "ab";
        let buffer = TextBuffer::from_str(file_buffer);
        assert!(buffer.get_char(0).unwrap() == 'a');
        assert!(buffer.get_char(1).unwrap() == 'b');
    }
    #[test]
    fn get_ii() {
        let file_buffer = "ac";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(1, 'b');
        assert!(res.is_ok());
        assert!(buffer.get_char(0).unwrap() == 'a');
        assert!(buffer.get_char(1).unwrap() == 'b');
        assert!(buffer.get_char(2).unwrap() == 'c');
    }
    #[test]
    fn delete_in_add() {
        let file_buffer = "B";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(0, 'A');
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                }
            ]
        );
        let res = buffer.delete(0);
        assert!(res.is_ok());
        assert_eq!(String::from("B"), generate_string(&buffer));
    }
    #[test]
    fn append() {
        let file_buffer = "a";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(1, 'b');
        assert!(res.is_ok());
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 1
                }
            ]
        );
        assert_eq!(String::from("ab"), generate_string(&buffer));
    }
    #[test]
    fn multiple_insertion_in_add_buffer() {
        let file_buffer = "";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.insert(0, 'a');
        assert!(res.is_ok());
        let res = buffer.insert(1, 'b');
        assert!(res.is_ok());
        let res = buffer.insert(2, 'c');
        assert!(res.is_ok());
        assert_eq!(String::from("abc"), generate_string(&buffer));
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 0
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 3
                }
            ]
        );
        let res = buffer.insert(1, '1');
        assert_eq!(
            buffer.piece_table,
            vec![
                PieceDescr {
                    buffer: BufferDescr::File,
                    offset: 0,
                    length: 0
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 0,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 3,
                    length: 1
                },
                PieceDescr {
                    buffer: BufferDescr::Add,
                    offset: 1,
                    length: 2
                }
            ]
        );

        assert_eq!(String::from("a1bc"), generate_string(&buffer));
        assert!(res.is_ok());
    }

    #[test]
    fn undo_delete_file() {
        let file_buffer = "AB";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(0);
        assert!(res.is_ok());

        let res = buffer.undo();
        assert!(res.is_ok());
        assert_eq!(String::from("AB"), generate_string(&buffer));
    }
    #[test]
    fn double_undo_delete_file() {
        let file_buffer = "AB";
        let mut buffer = TextBuffer::from_str(file_buffer);
        let res = buffer.delete(0);
        assert!(res.is_ok());

        let res = buffer.undo();
        assert!(res.is_ok());
        assert_eq!(String::from("AB"), generate_string(&buffer));
        let res = buffer.undo();
        assert!(res.is_ok());
        assert_eq!(String::from("AB"), generate_string(&buffer));
    }
}
