use core::mem::size_of;

use syscall::{syscall2, SystemCall};
use alloc::vec::Vec;

#[derive(Debug)]
#[repr(C, align(8))]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug)]
#[repr(C, u8)]
pub enum DrawCommand {
    DrawLine { from: Vertex, to: Vertex } = 0,
    DrawPolygon { vertices: Vec<Vertex> },
}

pub struct Drawer {}

impl Drawer {
    pub const fn new() -> Drawer { Drawer {} }

    pub fn draw(command: DrawCommand) {
        let command_addr = core::ptr::addr_of!(command) as usize;
        syscall2(SystemCall::WriteGraphic, command_addr, size_of::<DrawCommand>());
    }
}
