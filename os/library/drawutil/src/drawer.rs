use syscall::{syscall1, SystemCall};
use alloc::vec::Vec;
use alloc::vec;

#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
}

impl Vertex {
    pub fn new(x: u32, y: u32) -> Vertex {
        Self { x, y }
    }
}

#[repr(C, u8)]
pub enum DrawerCommand {
    CreatePanel = 0,
    ClosePanel,
    DrawLine { from: Vertex, to: Vertex },
    DrawPolygon(Vec<Vertex>),
    DrawCircle { center: Vertex, radius: u32 }
}

pub struct Drawer;

impl Drawer {
    pub const fn new() -> Drawer { Drawer }

    fn execute(command: DrawerCommand) {
        let command_addr = core::ptr::addr_of!(command) as usize;
        syscall1(SystemCall::WriteGraphic, command_addr);
    }

    pub fn draw_line(from: Vertex, to: Vertex) {
        let command = DrawerCommand::DrawLine { from , to };
        Drawer::execute(command);
    }

    pub fn draw_polygon(vertices: Vec<Vertex>) {
        let command = DrawerCommand::DrawPolygon(vertices);

        Drawer::execute(command);
    }

    pub fn draw_circle(center: (u32, u32), radius: u32) {
        let command = DrawerCommand::DrawCircle { center: Vertex::new(center.0, center.1), radius };

        Drawer::execute(command);
    }

    pub fn draw_rectangle(top_left: Vertex, bottom_right: Vertex) {
        let command = DrawerCommand::DrawPolygon(vec![
            Vertex::new(top_left.x, top_left.y),
            Vertex::new(bottom_right.x, top_left.y),
            Vertex::new(bottom_right.x, bottom_right.y),
            Vertex::new(top_left.x, bottom_right.y),
        ]);

        Drawer::execute(command);
    }

    pub fn draw_square(&self, top_left: Vertex, side_length: u32) {
        Drawer::draw_rectangle(
            top_left,
            Vertex::new(top_left.x + side_length, top_left.y + side_length),
        )
    }
}
