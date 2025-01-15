use crate::buffered_lfb;
use drawer::{drawer::DrawerCommand, rect_data::RectData};
use graphic::color::BLACK;

pub extern "C" fn sys_write_graphic(command_ptr: *const DrawerCommand) {
    let enum_val = unsafe { command_ptr.as_ref().unwrap() };
    let mut buff_lfb = buffered_lfb().lock();
    let lfb = buff_lfb.lfb();
    match enum_val {
        DrawerCommand::FullClearScreen(do_flush) => {
            lfb.clear();
            if *do_flush {
                buff_lfb.flush();
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
        },
        DrawerCommand::DrawCircle {
            center,
            radius,
            color,
        } => {
            lfb.draw_circle_bresenham((center.x as i32, center.y as i32), radius.clone() as i32, color.clone());
        },
        DrawerCommand::DrawFilledCircle {
            center,
            radius,
            inner_color,
            border_color
        } => {
            lfb.draw_filled_circle_bresenham((center.x as i32, center.y as i32), radius.clone() as i32, inner_color.clone());
        },
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
        },
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
        },
        DrawerCommand::DrawBitmap {
            bitmap,
            pos
        } => {
            lfb.draw_bitmap(pos.x, pos.y, &(**bitmap).data, (**bitmap).width, (**bitmap).height);
        },
        DrawerCommand::Flush => {
            buff_lfb.flush();
        }
    };
    
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
