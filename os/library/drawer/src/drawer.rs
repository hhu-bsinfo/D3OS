use core::marker::PhantomData;
use core::ops::Add;

use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use syscall::{syscall0, syscall1, SystemCall};

use graphic::color::Color;

#[repr(C, align(8))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
    private: PhantomData<()>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectData {
    pub top_left: Vertex,
    pub width: u32,
    pub height: u32,
}

impl Add for Vertex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x.saturating_add(rhs.x),
            y: self.y.saturating_add(rhs.y),
            private: PhantomData::default(),
        }
    }
}

impl Vertex {
    pub fn new(x: u32, y: u32) -> Self {
        Self {
            x,
            y,
            private: PhantomData::default(),
        }
    }

    pub fn as_tuple(&self) -> (u32, u32) {
        (self.x, self.y)
    }

    pub fn add(&self, x_delta: u32, y_delta: u32) -> Self {
        Self {
            x: self.x.saturating_add(x_delta),
            y: self.y.saturating_add(y_delta),
            private: PhantomData::default(),
        }
    }

    pub fn sub(&self, x_delta: u32, y_delta: u32) -> Self {
        Self {
            x: self.x.saturating_sub(x_delta),
            y: self.y.saturating_sub(y_delta),
            private: PhantomData::default(),
        }
    }
}

#[repr(C, u8)]
pub enum DrawerCommand {
    FullClearScreen = 0,
    PartialClearScreen {
        part_of_screen: RectData,
    },
    DrawLine {
        from: Vertex,
        to: Vertex,
        color: Color,
    },
    DrawPolygon {
        vertices: Vec<Vertex>,
        color: Color,
    },
    DrawCircle {
        center: Vertex,
        radius: u32,
        color: Color,
    },
    DrawFilledRectangle {
        rect_data: RectData,
        color: Color,
    },
    DrawFilledTriangle {
        vertices: [Vertex; 3],
        color: Color,
    },
    DrawChar {
        char_to_draw: char,
        pos: Vertex,
        color: Color,
        scale: (u32, u32),
    },
    DrawString {
        string_to_draw: String,
        pos: Vertex,
        color: Color,
        scale: (u32, u32),
    },
}

pub struct Drawer;

impl Drawer {
    fn execute(command: DrawerCommand) {
        let command_addr = core::ptr::addr_of!(command) as usize;
        syscall1(SystemCall::WriteGraphic, command_addr);
    }

    pub fn full_clear_screen() {
        let command = DrawerCommand::FullClearScreen;

        Self::execute(command);
    }

    pub fn partial_clear_screen(part_of_screen: RectData) {
        let command = DrawerCommand::PartialClearScreen { part_of_screen };

        Self::execute(command);
    }

    pub fn get_graphic_resolution() -> (u32, u32) {
        let raw_graphic_resolution: usize = syscall0(SystemCall::GetGraphicResolution);
        return (
            (raw_graphic_resolution >> 32) as u32,
            raw_graphic_resolution as u32,
        );
    }

    pub fn draw_line(from: Vertex, to: Vertex, color: Color) {
        let command = DrawerCommand::DrawLine { from, to, color };
        Self::execute(command);
    }

    pub fn draw_polygon(vertices: Vec<Vertex>, color: Color) {
        let command = DrawerCommand::DrawPolygon { vertices, color };

        Self::execute(command);
    }

    pub fn draw_circle(center: Vertex, radius: u32, color: Color) {
        let command = DrawerCommand::DrawCircle {
            center,
            radius,
            color,
        };

        Self::execute(command);
    }

    pub fn draw_filled_rectangle(rect_data: RectData, color: Color) {
        let command = DrawerCommand::DrawFilledRectangle { rect_data, color };

        Self::execute(command);
    }

    pub fn draw_filled_triangle(vertices: [Vertex; 3], color: Color) {
        let command = DrawerCommand::DrawFilledTriangle { vertices, color };

        Self::execute(command);
    }

    pub fn draw_char(char_to_draw: char, pos: Vertex, color: Color, scale: (u32, u32)) {
        let command = DrawerCommand::DrawChar {
            char_to_draw,
            pos,
            color,
            scale,
        };

        Self::execute(command);
    }

    pub fn draw_string(string_to_draw: String, pos: Vertex, color: Color, scale: (u32, u32)) {
        let command = DrawerCommand::DrawString {
            string_to_draw,
            pos,
            color,
            scale,
        };

        Self::execute(command);
    }

    pub fn draw_rectangle(top_left: Vertex, bottom_right: Vertex, color: Color) {
        let command = DrawerCommand::DrawPolygon {
            vertices: vec![
                Vertex::new(top_left.x, top_left.y),
                Vertex::new(bottom_right.x, top_left.y),
                Vertex::new(bottom_right.x, bottom_right.y),
                Vertex::new(top_left.x, bottom_right.y),
            ],
            color,
        };

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
