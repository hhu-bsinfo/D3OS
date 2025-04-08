/*
    Communicates with the kernel to read mouse data
    and decodes it into packets.
*/

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

    pub struct IntelliMouseFlags: u8 {
        const BUTTON_4 = 0x01;
        const BUTTON_5 = 0x02;
        const _ = 0x04;
        const _ = 0x08;
    }
}

pub struct MousePacket {
    pub flags: MouseFlags,
    pub dx: i16,
    pub dy: i16,
    pub dz : i8,
    pub im_flags: IntelliMouseFlags,
}

impl MousePacket {
    pub fn from_u32(value: u32) -> Self {
        let flags = (value >> 0) as u8; // byte 1
        let dx = (value >> 8) as u8;    // byte 2
        let dy = (value >> 16) as u8;   // byte 3
        let im = (value >> 24) as u8;   // byte 4

        // Subtract 0x100 from dx and dy if the sign bit is set
        let dx: i16 = (dx as i16) - (((flags as i16) << 4) & 0x100);
        let dy: i16 = (dy as i16) - (((flags as i16) << 3) & 0x100);

        // Read scroll wheel movement (4 bits signed)
        let dz: u8 = im & 0x0F;
        let dz = (dz as i8) << 4 >> 4;

        // Read standard flags
        let flags = MouseFlags::from_bits_truncate(flags);

        // Read intellimouse flags (4 bits)
        let im_flags = (im >> 4) as u8;
        let im_flags = IntelliMouseFlags::from_bits_truncate(im_flags);

        Self {
            flags,
            dx,
            dy,
            dz,
            im_flags,
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

    pub fn button4_down(&self) -> bool {
        self.im_flags.contains(IntelliMouseFlags::BUTTON_4)
    }
    
    pub fn button5_down(&self) -> bool {
        self.im_flags.contains(IntelliMouseFlags::BUTTON_5)
    }
}

pub fn try_read_mouse() -> Option<MousePacket> {
    let res = syscall(SystemCall::MouseRead, &[]);

    match res {
        Ok(0) => None,
        Ok(value) => Some(MousePacket::from_u32(value as u32)),
        Err(_) => None,
    }
}
