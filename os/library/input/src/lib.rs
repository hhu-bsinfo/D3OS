#![no_std]

use bitflags::bitflags;
use syscall::{SystemCall, syscall};

bitflags! {
    pub struct MouseFlags: u8 {
        const LEFT_BUTTON = 0x01;
        const RIGHT_BUTTON = 0x02;
        const MIDDLE_BUTTON = 0x04;
        const ALWAYS_ON = 0x08;
        const X_SIGNED = 0x10;
        const Y_SIGNED = 0x20;
        const X_OVERFLOW = 0x40;
        const Y_OVERFLOW = 0x80;
    }
}

pub struct MousePacket {
    pub flags: MouseFlags,
    pub dx: i16,
    pub dy: i16,
}

impl MousePacket {
    pub fn from_i32(value: i32) -> Self {
        let flags = (value >> 0) as u8; // byte 0
        let dx = (value >> 8) as u8;    // byte 1
        let dy = (value >> 16) as u8;   // byte 2
        let _ = (value >> 24) as u8;        // byte 3 (unused)

        // Subtract 0x100 from dx and dy if the sign bit is set
        let dx: i16 = (dx as i16) - (((flags as i16) << 4) & 0x100);
        let dy: i16 = (dy as i16) - (((flags as i16) << 3) & 0x100);

        Self {
            flags: MouseFlags::from_bits_truncate(flags),
            dx,
            dy,
        }
    }

    pub fn left_button_down(&self) -> bool {
        self.flags.contains(MouseFlags::LEFT_BUTTON)
    }

    pub fn right_button_down(&self) -> bool {
        self.flags.contains(MouseFlags::RIGHT_BUTTON)
    }

    pub fn middle_button_down(&self) -> bool {
        self.flags.contains(MouseFlags::MIDDLE_BUTTON)
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
