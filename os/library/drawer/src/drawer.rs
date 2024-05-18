use syscall::{syscall0, syscall1, SystemCall};
use alloc::vec::Vec;
use alloc::vec;

use graphic::color::Color;

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
}

impl Vertex {
    pub fn new(x: u32, y: u32) -> Vertex {
        Self { x, y }
    }

    pub fn as_tuple(&self) -> (u32, u32) {
        (self.x, self.y)
    }
}

#[repr(C, u8)]
pub enum DrawerCommand {
    ClearScreen = 0,
    DeleteContext,
    DrawLine { from: Vertex, to: Vertex, color: Color },
    DrawPolygon(Vec<Vertex>, Color),
    DrawCircle { center: Vertex, radius: u32, color: Color }
}

pub struct Drawer;

impl Drawer {
    fn execute(command: DrawerCommand) {
        let command_addr = core::ptr::addr_of!(command) as usize;
        syscall1(SystemCall::WriteGraphic, command_addr);
    }

    pub fn clear_screen() {
        let command = DrawerCommand::ClearScreen;

        Self::execute(command);
    }

    pub fn delete_context() {
        let command = DrawerCommand::DeleteContext;

        Self::execute(command);
    }

    pub fn get_graphic_resolution() -> (u32, u32) {
        let raw_graphic_resolution: usize = syscall0(SystemCall::GetGraphicResolution);
        return ((raw_graphic_resolution >> 32) as u32, raw_graphic_resolution as u32);
    }

    pub fn draw_line(from: Vertex, to: Vertex, color: Color) {
        let command = DrawerCommand::DrawLine { from , to, color };
        Self::execute(command);
    }

    pub fn draw_polygon(vertices: Vec<Vertex>, color: Color) {
        let command = DrawerCommand::DrawPolygon(vertices, color);

        Self::execute(command);
    }

    pub fn draw_circle(center: Vertex, radius: u32, color: Color) {
        let command = DrawerCommand::DrawCircle { center, radius, color };

        Self::execute(command);
    }

    pub fn draw_rectangle(top_left: Vertex, bottom_right: Vertex, color: Color) {
        let command = DrawerCommand::DrawPolygon(vec![
            Vertex::new(top_left.x, top_left.y),
            Vertex::new(bottom_right.x, top_left.y),
            Vertex::new(bottom_right.x, bottom_right.y),
            Vertex::new(top_left.x, bottom_right.y),
        ],
        color);

        Self::execute(command);
    }

    pub fn draw_square(top_left: Vertex, side_length: u32, color: Color) {
        Self::draw_rectangle(
            top_left,
            Vertex::new(top_left.x + side_length, top_left.y + side_length),
            color,
        )
    }
}
