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
    let command = DrawerCommand::DrawLine { from: Vertex::new(150, 100) , to: Vertex::new(400, 600) };
    Drawer::execute(command);
}

fn draw_polygon() {
    let command = DrawerCommand::DrawPolygon(vec![
        Vertex::new(3, 200),
        Vertex::new(30, 150),
        Vertex::new(200, 200),
        Vertex::new(123, 123),
        Vertex::new(230, 80),
    ]);

    Drawer::execute(command);
}

fn draw_circle() {
    let command = DrawerCommand::DrawCircle { center: Vertex::new(400, 400), radius: 50 };

    Drawer::execute(command);
}