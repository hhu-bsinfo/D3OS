use log::{debug, error};

const WRITE_BUFFER_SIZE: usize = 128;

#[derive(Debug)]
pub struct Tty {
    write_index: usize,
    write_buffer: [u8; WRITE_BUFFER_SIZE],
    read_buffer: u8,
    is_reading: bool,
}

impl Tty {
    pub const fn new() -> Self {
        Self {
            write_index: 0,
            write_buffer: [0; WRITE_BUFFER_SIZE],
            read_buffer: 0,
            is_reading: false,
        }
    }

    pub fn produce_read(&mut self, byte: u8) {
        self.read_buffer = byte;
    }

    pub fn consume_read(&mut self) -> u8 {
        let byte = self.read_buffer;
        self.read_buffer = 0;
        byte
    }

    pub fn can_read(&self) -> bool {
        self.read_buffer > 0
    }

    pub fn is_reading(&self) -> bool {
        self.is_reading
    }

    pub fn start_reading(&mut self) {
        self.is_reading = true
    }

    pub fn stop_reading(&mut self) {
        self.is_reading = false;
        self.read_buffer = 0;
    }

    pub fn push_write(&mut self, buffer: &[u8]) -> Result<(), ()> {
        let free = self.write_buffer.len() - self.write_index;
        if free < buffer.len() {
            error!(
                "Unable to write string, buffer length exceeded (length: {}, free: {}, received: {})",
                self.write_buffer.len(),
                free,
                buffer.len()
            );
            return Err(());
        };

        let end = self.write_index + buffer.len();
        self.write_buffer[self.write_index..end].copy_from_slice(buffer);
        self.write_index = end;

        Ok(())
    }

    pub fn consume_write(&mut self) -> [u8; WRITE_BUFFER_SIZE] {
        let copy = self.write_buffer;
        self.write_buffer.fill(0);
        self.write_index = 0;
        copy
    }

    pub fn write_len(&self) -> usize {
        self.write_buffer.len()
    }

    pub fn write_index(&self) -> usize {
        self.write_index
    }
}
