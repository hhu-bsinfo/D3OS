use log::error;

const WRITE_BUFFER_SIZE: usize = 128;

#[derive(Debug)]
pub struct Tty {
    write_index: usize,
    write_buffer: [u8; WRITE_BUFFER_SIZE],
}

impl Tty {
    pub const fn new() -> Self {
        Self {
            write_index: 0,
            write_buffer: [0; WRITE_BUFFER_SIZE],
        }
    }

    pub fn push_write(&mut self, buffer: &[u8]) -> Result<(), ()> {
        let free = self.write_buffer.len() - self.write_index;
        if free < buffer.len() {
            error!(
                "Unable to write string, buffer length exceeded (free: {}, received: {})",
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
