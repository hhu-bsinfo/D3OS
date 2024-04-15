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

    pub fn draw_line(from: (u32, u32), to: (u32, u32)) {
        let command = DrawerCommand::DrawLine { from: Vertex::new(from.0, from.1) , to: Vertex::new(to.0, to.1) };
        Drawer::execute(command);
    }

    pub fn draw_polygon(vertices_as_tuples: Vec<(u32, u32)>) {
        let vertices: Vec<Vertex> = vertices_as_tuples
            .into_iter()
            .map(|tuple| Vertex::new(tuple.0, tuple.1))
            .collect();
        let command = DrawerCommand::DrawPolygon(vertices);

        Drawer::execute(command);
    }

    pub fn draw_circle(center: (u32, u32), radius: u32) {
        let command = DrawerCommand::DrawCircle { center: Vertex::new(center.0, center.1), radius };

        Drawer::execute(command);
    }

    pub fn draw_rectangle(upper_left_vertex: (u32, u32), lower_right_vertex: (u32, u32)) {
        let command = DrawerCommand::DrawPolygon(vec![
            Vertex::new(upper_left_vertex.0, upper_left_vertex.1),
            Vertex::new(lower_right_vertex.0, upper_left_vertex.1),
            Vertex::new(lower_right_vertex.0, lower_right_vertex.1),
            Vertex::new(upper_left_vertex.0, lower_right_vertex.1),
        ]);

        Drawer::execute(command);
    }

    pub fn draw_square(upper_left_vertex: (u32, u32), side_length: u32) {
        Drawer::draw_rectangle(
            upper_left_vertex,
            (upper_left_vertex.0 + side_length, upper_left_vertex.1 + side_length),
        )
    }
}
