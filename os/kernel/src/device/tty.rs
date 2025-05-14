use alloc::collections::vec_deque::VecDeque;

#[derive(Debug)]
pub struct TtyInput {
    buffer: u8,
    has_reader: bool,
}

#[derive(Debug)]
pub struct TtyOutput {
    buffer: VecDeque<u8>,
    current_index: usize,
}

impl TtyInput {
    pub const fn new() -> Self {
        Self {
            buffer: 0,
            has_reader: false,
        }
    }

    pub fn write(&mut self, byte: u8) {
        self.buffer = byte;
    }

    pub fn read(&mut self) -> u8 {
        let byte = self.buffer;
        self.buffer = 0;
        byte
    }

    pub fn start_read(&mut self) {
        self.has_reader = true;
    }

    pub fn end_read(&mut self) {
        self.has_reader = false;
    }

    pub fn can_write(&self) -> bool {
        self.has_reader == true
    }

    pub fn can_read(&self) -> bool {
        self.buffer > 0
    }
}

impl TtyOutput {
    pub const fn new() -> Self {
        Self {
            buffer: VecDeque::new(),
            current_index: 0,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) -> usize {
        let mut count = 0;
        for byte in bytes {
            self.buffer.push_back(*byte);
            count += 1;
        }

        count
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        let mut count = 0;
        for byte in buffer {
            *byte = match self.buffer.pop_front() {
                Some(read_byte) => {
                    count += 1;
                    read_byte
                }
                None => break,
            };
        }

        count
    }
}
