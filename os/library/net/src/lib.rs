#![no_std]

#[derive(Debug)]
#[repr(u8)]
#[non_exhaustive]
pub enum SocketType {
    Udp, Tcp, Icmp,
}

impl From<SocketType> for usize {
    fn from(value: SocketType) -> Self {
        (value as u8) as usize
    }
}