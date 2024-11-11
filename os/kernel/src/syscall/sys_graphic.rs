use drawer::{drawer::DrawerCommand, rect_data::RectData};
use graphic::color::BLACK;
use libm::Libm;
use core::f32::consts::PI;
use core::ptr::slice_from_raw_parts;
use core::str::from_utf8;
use crate::{
    buffered_lfb,
};
use log::debug;

#[unsafe(no_mangle)]
pub extern "C" fn sys_write_graphic(command_ptr: *const DrawerCommand) {
    let enum_val = unsafe { command_ptr.as_ref().unwrap() };
    let mut buff_lfb = buffered_lfb().lock();
    let lfb = buff_lfb.lfb();
    match enum_val {
        DrawerCommand::FullClearScreen(do_flush) => {
            lfb.clear();
            if !do_flush {
                return;
            }
        }
        DrawerCommand::DrawLine { from, to, color } => {
            lfb.draw_line(from.x, from.y, to.x, to.y, color.clone())
        }
        DrawerCommand::DrawPolygon { vertices, color } => {
            let first_vertex = vertices.first();
            let mut prev = match first_vertex {
                Some(unwrapped) => unwrapped,
                None => return,
            };
            let last_vertex = vertices.last().unwrap();
            for vertex in &vertices[1..] {
                lfb.draw_line(prev.x, prev.y, vertex.x, vertex.y, color.clone());
                prev = vertex;
            }

            lfb.draw_line(
                last_vertex.x,
                last_vertex.y,
                first_vertex.unwrap().x,
                first_vertex.unwrap().y,
                color.clone(),
            );
        }
        DrawerCommand::DrawFilledRectangle {
            rect_data:
                RectData {
                    top_left,
                    width,
                    height,
                },
            inner_color,
            border_color,
        } => match border_color {
            Some(border_color) => {
                let border_width = 3;
                lfb.fill_rect(top_left.x, top_left.y, *width, *height, *border_color);
                lfb.fill_rect(
                    top_left.x + border_width,
                    top_left.y + border_width,
                    *width - 2 * border_width,
                    *height - 2 * border_width,
                    *inner_color,
                );
            }
            None => {
                lfb.fill_rect(top_left.x, top_left.y, *width, *height, *inner_color);
            }
        },
        DrawerCommand::DrawFilledTriangle { vertices, color } => {
            let tuples = vertices.map(|vertex| vertex.as_tuple());
            lfb.fill_triangle((tuples[0], tuples[1], tuples[2]), *color)
        }

        DrawerCommand::DrawCircle {
            center,
            radius,
            color,
        } => {
            let stepsize = PI / 128.0;
            const TWO_PI: f32 = PI * 2.0;
            let mut x_curr = 0.0;
            while x_curr <= TWO_PI {
                lfb.draw_pixel(
                    Libm::<f32>::round(
                        Libm::<f32>::sin(x_curr) * (radius.clone() as f32) + (center.x as f32),
                    ) as u32,
                    Libm::<f32>::round(
                        Libm::<f32>::cos(x_curr) * (radius.clone() as f32) + (center.y as f32),
                    ) as u32,
                    color.clone(),
                );

                x_curr += stepsize;
            }
        }
        DrawerCommand::DrawString {
            string_to_draw,
            pos,
            fg_color,
            bg_color,
            scale,
        } => {
            lfb.draw_string_scaled(
                pos.x,
                pos.y,
                scale.0,
                scale.1,
                fg_color.clone(),
                bg_color.clone(),
                string_to_draw,
            );
        }
        DrawerCommand::DrawChar {
            char_to_draw,
            pos,
            color,
            scale,
        } => {
            lfb.draw_char_scaled(
                pos.x,
                pos.y,
                scale.0,
                scale.1,
                color.clone(),
                BLACK,
                *char_to_draw,
            );
        }
        DrawerCommand::PartialClearScreen { part_of_screen } => {
            lfb.fill_rect(
                part_of_screen.top_left.x,
                part_of_screen.top_left.y,
                part_of_screen.width,
                part_of_screen.height,
                BLACK,
            );
        }
    };

    buff_lfb.flush();
}

/// w = width, h = height;
/// Format in bytes: wwwwhhhh
pub extern "C" fn sys_get_graphic_resolution() -> usize {
    // We need 64bits to transform the information of both width and height.
    if size_of::<usize>() != 8 {
        return 0;
    }
    let buffered_lfb = &mut buffered_lfb().lock();
    let lfb = buffered_lfb.direct_lfb();
    return (((lfb.width() as u64) << 32) | (lfb.height() as u64)) as usize;
}

pub extern "C" fn sys_log_serial(string_addr: *const u8, string_len: usize) {
    let log_string = from_utf8(unsafe {
        slice_from_raw_parts(string_addr, string_len)
            .as_ref()
            .unwrap()
    })
    .unwrap();

    debug!("{}", log_string);
}
 