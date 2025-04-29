/*
	
	piece table text buffer  --  Julius Drodofsky

	from_str()
	delete(logical_adress)
	insert(logical_adress, char)
	to_string()
	get_char()
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
}

impl<'s> TextBuffer<'s> {
    // returns (index to piecetable entry,  possition in piece_descr_span (offset<=i<length))
    fn resolve_logical_adress(&self, logical_adress: usize) -> Option<(usize, usize)> {
        let mut piece_table_index = 0;
        let mut la_start = 0;
        while let Some(piece_descr) = self.piece_table.get(piece_table_index) {
            if logical_adress >= la_start && logical_adress < la_start + piece_descr.length {
                return Some((piece_table_index, logical_adress - la_start));
            }
            la_start += piece_descr.length;
            piece_table_index += 1;
        }
        None
    }

    pub fn delete(&mut self, logical_adress: usize) -> Result<(), TextBufferError> {
        let (piece_table_index, piece_descr_offset) =
            match self.resolve_logical_adress(logical_adress) {
                Some((i, o)) => (i, o),
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
        Ok(())
    }

    pub fn from_str(file_buffer: &'s str) -> Self {
        Self {
            file_buffer,
            add_buffer: String::new(),
            piece_table: vec![PieceDescr::new(BufferDescr::File, 0, file_buffer.len())],
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
        assert_eq!(buffer.resolve_logical_adress(0), Some((0, 0)));
        assert_eq!(buffer.resolve_logical_adress(1), Some((0, 1)));
        assert_eq!(buffer.resolve_logical_adress(2), Some((0, 2)));
        assert_eq!(buffer.resolve_logical_adress(3), Some((0, 3)));
        assert_eq!(buffer.resolve_logical_adress(4), Some((1, 0)));
        assert_eq!(buffer.resolve_logical_adress(5), Some((1, 1)));
        assert_eq!(buffer.resolve_logical_adress(6), Some((1, 2)));
        assert_eq!(buffer.resolve_logical_adress(7), Some((1, 3)));
        assert_eq!(buffer.resolve_logical_adress(8), Some((1, 4)));
        assert_eq!(buffer.resolve_logical_adress(9), Some((1, 5)));
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
        let res = buffer.delete(0);
        assert!(res.is_ok());
        assert_eq!(buffer.piece_table, vec![]);
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
        let ret = buffer.resolve_logical_adress(4);
        assert_eq!(ret, None);
    }
}
