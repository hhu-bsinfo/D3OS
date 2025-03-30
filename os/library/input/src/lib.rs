#![no_std]

use syscall::{syscall, SystemCall};

pub struct MousePacket {
    pub flags: u8,
    pub dx: i16,
    pub dy: i16,
}

impl MousePacket {
    pub fn from_i32(value: i32) -> Self {
        let flags = (value >> 0) as u8;
        let dx = (value >> 8) as u8;
        let dy = (value >> 16) as u8;
        let _ = (value >> 24) as u8;

        let dx : i16 = (dx as i16) - (((flags as i16) << 4) & 0x100);
        let dy : i16 = (dy as i16) - (((flags as i16) << 5) & 0x100);
        /*if flags.contains(MouseFlags::X_SIGNED) {
            dx -= 0x100;
        }

        if flags.contains(MouseFlags::Y_SIGNED) {
            dy -= 0x100;
        }*/

        Self { flags, dx, dy }
    }
}

pub fn try_read_mouse() -> Option<MousePacket> {
    let res = syscall(SystemCall::MouseRead, &[]);

    match res {
        Ok(0) => None,
        Ok(value) => Some(MousePacket::from_i32(value as i32)),
        Err(_) => None,
    }
}
