#![no_std]

#[repr(u8)]
pub enum SocketType {
    Udp
}

impl From<SocketType> for usize {
    fn from(value: SocketType) -> Self {
        (value as u8) as usize
    }
}