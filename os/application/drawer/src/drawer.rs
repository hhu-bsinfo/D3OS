#![no_std]

extern crate alloc;

use alloc::vec;
use drawutil::drawer::{DrawerCommand, Drawer, Vertex};
#[allow(unused_imports)]
use runtime::*;

#[no_mangle]
pub fn main() {
    Drawer::execute(DrawerCommand::CreatePanel);
    draw_circle();
}

fn draw_line() {
    let command = DrawerCommand::DrawLine { from: Vertex {x: 150, y: 100}, to: Vertex {x: 400, y: 600} };
    Drawer::execute(command);
}

fn draw_polygon() {
    let command = DrawerCommand::DrawPolygon(vec![
        Vertex {x: 3, y: 200},
        Vertex {x: 30, y: 150},
        Vertex {x: 200, y: 200},
        Vertex {x: 123, y: 123},
        Vertex {x: 230, y: 80}
    ]);

    Drawer::execute(command);
}

fn draw_circle() {
    let command = DrawerCommand::DrawCircle { center: Vertex { x: 400, y: 400 }, radius: 50 };

    Drawer::execute(command);
}