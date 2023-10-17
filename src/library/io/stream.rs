use alloc::string::String;

pub trait InputStream {
    fn read_byte(&mut self) -> i16;
}

pub trait OutputStream {
    fn write_byte(&mut self, b: u8);
    fn write_str(&mut self, string: &String);
}