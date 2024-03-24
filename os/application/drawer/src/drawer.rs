#![no_std]

extern crate alloc;

use drawutil::drawer::{DrawerCommand, Drawer, Vertex};
#[allow(unused_imports)]
use runtime::*;

#[no_mangle]
pub fn main() {
    Drawer::execute(DrawerCommand::CreatePanel);
    let command = DrawerCommand::DrawLine { from: Vertex {x: 0, y: 2}, to: Vertex {x: 200, y: 500} };
    Drawer::execute(command);
}