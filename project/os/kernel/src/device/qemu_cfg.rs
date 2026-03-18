use crate::device::qemu_cfg::Selector::Signature;
use x86_64::instructions::port::{PortReadOnly, PortWriteOnly};

const SELECTOR_PORT: u16 = 0x510;
const DATA_PORT: u16 = 0x511;

#[allow(dead_code)]
#[repr(u16)]
enum Selector {
    Signature = 0x0000,
    Id = 0x0001,
    RootDirectory = 0x0019,
}

pub fn is_available() -> bool {
    let mut selector_port = PortWriteOnly::<u16>::new(SELECTOR_PORT);
    let mut data_port = PortReadOnly::<u8>::new(DATA_PORT);
    let id: [u8; 4];

    unsafe {
        selector_port.write(Signature as u16);
        id = [
            data_port.read(),
            data_port.read(),
            data_port.read(),
            data_port.read(),
        ]
    }

    id[0] == b'Q' && id[1] == b'E' && id[2] == b'M' && id[3] == b'U'
}
