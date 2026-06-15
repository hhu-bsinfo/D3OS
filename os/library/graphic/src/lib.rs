#![no_std]

extern crate alloc;

use core::ptr;
use syscall::{syscall, SystemCall};

pub mod ansi;
pub mod buffered_lfb;
pub mod color;
pub mod lfb;

pub struct FramebufferInfo {
    pub addr: u64,
    pub width: u32,
    pub height: u32,
    pub pitch: u32,
    pub bpp: u8
}

pub fn map_framebuffer() -> Result<FramebufferInfo, ()> {
    let mut fb_info = FramebufferInfo {
        addr: 0,
        width: 0,
        height: 0,
        pitch: 0,
        bpp: 0
    };

    match syscall(SystemCall::MapFrameBuffer, &[ptr::from_mut(&mut fb_info) as usize]) {
        Ok(_) => Ok(fb_info),
        Err(_) => Err(())
    }
}