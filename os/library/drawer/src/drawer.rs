use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use syscall::{syscall0, syscall1, SystemCall};

use graphic::color::{Color, INVISIBLE};

use crate::{rect_data::RectData, vertex::Vertex};

#[repr(C, u8)]
pub enum DrawerCommand {
    FullClearScreen(bool) = 0,
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
        inner_color: Color,
        border_color: Option<Color>,
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
        fg_color: Color,
        bg_color: Color,
        scale: (u32, u32),
    },
    Flush,
}

pub struct Drawer;

impl Drawer {
    fn execute(command: DrawerCommand) {
        let command_addr = core::ptr::addr_of!(command) as usize;
        syscall1(SystemCall::WriteGraphic, command_addr);
    }

    /**
    If `do_flush` is `true``, we flush and have an empty screen for at least one frame.
    Use `false`, if you wanna draw something new, to minimize screen flickering
    */
    pub fn full_clear_screen(do_flush: bool) {
        let command = DrawerCommand::FullClearScreen(do_flush);

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

    pub fn draw_filled_rectangle(
        rect_data: RectData,
        inner_color: Color,
        border_color: Option<Color>,
    ) {
        let command = DrawerCommand::DrawFilledRectangle {
            rect_data,
            inner_color,
            border_color,
        };

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

    pub fn draw_string(
        string_to_draw: String,
        pos: Vertex,
        fg_color: Color,
        bg_color: Option<Color>,
        scale: (u32, u32),
    ) {
        let command = DrawerCommand::DrawString {
            string_to_draw,
            pos,
            fg_color,
            bg_color: bg_color.unwrap_or(INVISIBLE),
            scale,
        };

        Self::execute(command);
    }

    pub fn draw_rectangle(rect_data: RectData, color: Color) {
        let RectData {
            top_left,
            width,
            height,
        } = rect_data;
        let bottom_right = Vertex::new(top_left.x + width, top_left.y + height);

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
            RectData {
                top_left,
                width: side_length,
                height: side_length,
            },
            color,
        )
    }

    pub fn flush() {
        Self::execute(DrawerCommand::Flush);
    }
}
