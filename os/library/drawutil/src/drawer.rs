use core::mem::size_of;

use syscall::{syscall2, SystemCall};
use alloc::vec::Vec;

#[repr(C, align(8))]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
}

#[repr(C, u8)]
pub enum DrawerCommand {
    CreatePanel = 0,
    ClosePanel,
    DrawLine { from: Vertex, to: Vertex },
    DrawPolygon(Vec<Vertex>),
}

pub struct Drawer;

impl Drawer {
    pub const fn new() -> Drawer { Drawer }

    pub fn execute(command: DrawerCommand) {
        let command_addr = core::ptr::addr_of!(command) as usize;
        syscall2(SystemCall::WriteGraphic, command_addr, size_of::<DrawerCommand>());
    }
}
