use core::{cell::RefCell, ptr};

use alloc::{format, string::ToString};
use anstyle_parse::{Params, ParamsIter, Parser, Perform, Utf8Parser};
use concurrent::{
    process,
    thread::{self},
};
use graphic::{
    ansi::COLOR_TABLE_256,
    color::{self, Color, INVISIBLE},
    lfb,
};
use input::keyboard;

use pc_keyboard::KeyEvent;
use spin::Mutex;
use stream::{RawInputStream, OutputStream};
use time::{date, systime};

use crate::{util::system_info::system_info, worker::cursor::CursorState};

use super::{
    color::ColorState,
    display::{Character, DisplayState},
    terminal::Terminal,
};

const TAB_SPACES: u16 = 4;

pub struct LFBTerminal {
    pub(crate) display: Mutex<DisplayState>,
    pub(crate) cursor: Mutex<CursorState>,
    pub(crate) color: Mutex<ColorState>,
    pub(crate) parser: Mutex<RefCell<Parser>>,
}

unsafe impl Send for LFBTerminal {}
unsafe impl Sync for LFBTerminal {}

impl OutputStream for LFBTerminal {
    fn write_byte(&self, b: u8) {
        let parser = self.parser.lock();
        // advance() passes a mutable terminal reference to methods in 'Perform' trait,
        // but for LFBTerminal, none of these methods actually need a mutable reference,
        // so it is safe to just construct a mutable reference here.
        unsafe {
            parser
                .borrow_mut()
                .advance(ptr::from_ref(self).cast_mut().as_mut().unwrap(), b);
        }
    }

    fn write_str(&self, string: &str) {
        let parser = self.parser.lock();
        for b in string.bytes() {
            // advance() passes a mutable terminal reference to methods in 'Perform' trait,
            // but for LFBTerminal, none of these methods actually need a mutable reference,
            // so it is safe to just construct a mutable reference here.
            unsafe {
                parser
                    .borrow_mut()
                    .advance(ptr::from_ref(self).cast_mut().as_mut().unwrap(), b);
            }
        }
    }
}

impl RawInputStream for LFBTerminal {
    fn read_event(&self) -> KeyEvent {
        keyboard::read_raw(true).unwrap()
    }
    
    fn read_event_nb(&self) -> Option<KeyEvent> {
        keyboard::read_raw(false)
    }
}

impl Terminal for LFBTerminal {
    fn clear(&self) {
        let mut display = self.display.lock();
        let mut cursor = self.cursor.lock();
        let mut color = self.color.lock();

        LFBTerminal::clear_screen(&mut display, &mut color);
        LFBTerminal::position(&mut display, &mut cursor, &mut color, (0, 0));
    }
}

impl LFBTerminal {
    pub fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        Self {
            display: Mutex::new(DisplayState::new(buffer, pitch, width, height, bpp)),
            cursor: Mutex::new(CursorState::new()),
            color: Mutex::new(ColorState::new()),
            parser: Mutex::new(RefCell::new(Parser::<Utf8Parser>::new())),
        }
    }

    fn print_char(&self, c: char) {
        let mut display = self.display.lock();
        let mut cursor = self.cursor.lock();
        let mut color = self.color.lock();

        if c == '\n' {
            LFBTerminal::clear_line_from_cursor(&mut display, &mut cursor, &mut color);

            cursor.pos.0 = 0;
            cursor.pos.1 += 1;
        } else if c == 0x08 as char { // backspace key
            // Store old cursor position and character before moving it one position left
            let old_pos = cursor.pos;
            let old_index = (old_pos.1 * display.size.0 + old_pos.0) as usize;
            let old_char_value = match display.char_buffer[old_index].value {
                '\0' => ' ',
                value => value,
            };
            
            // Create new character to overwrite the character to the left of the cursor
            let new_char = Character { value: ' ', fg_color: color.fg_color, bg_color: color.bg_color };
            
            // Move the cursor one position to the left
            cursor.pos.0 -= 1;
            let new_index = (cursor.pos.1 * display.size.0 + cursor.pos.0) as usize;
            
            display.char_buffer[new_index] = new_char; // Remove character at the new location from the character buffer
            
            /* Restore character display at the old cursor position before moving.
             * This is required, as the cursor might have been blinking before moving,
             * leaving a filled rectangle at the old position.
             */
            LFBTerminal::print_char_at(&mut display, &mut color, old_char_value, old_pos);
            
            // Remove character at the new position from the screen
            LFBTerminal::print_char_at(&mut display, &mut color, new_char.value, cursor.pos);
        } else {
            let char_width = LFBTerminal::print_char_at(&mut display, &mut color, c, cursor.pos);
            if char_width > 0 {
                let index = (cursor.pos.1 * display.size.0 + cursor.pos.0) as usize;
                let char_columns = (char_width / lfb::DEFAULT_CHAR_WIDTH
                    + (if char_width % lfb::DEFAULT_CHAR_WIDTH == 0 {
                        0
                    } else {
                        1
                    })) as u16;

                // Set character in character buffer
                display.char_buffer[index] = Character {
                    value: c,
                    fg_color: color.fg_color,
                    bg_color: color.bg_color,
                };

                // Null out following, if glyph is larger than one column
                for i in 1..char_columns {
                    if cursor.pos.0 + i >= display.size.0 {
                        break;
                    }

                    display.char_buffer[index + i as usize] = Character {
                        value: '\0',
                        fg_color: INVISIBLE,
                        bg_color: INVISIBLE,
                    };
                }

                if cursor.pos.0 + char_columns >= display.size.0 {
                    let row = cursor.pos.1;
                    LFBTerminal::position(&mut display, &mut cursor, &mut color, (0, row + 1));
                } else {
                    cursor.pos.0 += char_columns;
                }
            }
        }

        if cursor.pos.0 >= display.size.0 {
            cursor.pos.1 += 1;
            cursor.pos.0 = 0;
        }

        if cursor.pos.1 >= display.size.1 {
            LFBTerminal::scroll_up(&mut display, &mut color);
            cursor.pos.0 = 0;
            cursor.pos.1 = display.size.1 - 1;
            let pos = (0, display.size.1);

            LFBTerminal::print_char_at(&mut display, &mut color, '_', pos);
        }
    }

    fn print_char_at(display: &mut DisplayState, color: &mut ColorState, c: char, pos: (u16, u16)) -> u32 {
        display.lfb.lfb().draw_char(
            pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            color.fg_color,
            color.bg_color,
            c,
        );
        display.lfb.direct_lfb().draw_char(
            pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            color.fg_color,
            color.bg_color,
            c,
        )
    }

    pub fn draw_status_bar(display: &mut DisplayState) {
        // Draw background
        for i in 0..display.size.0 as u32 * lfb::DEFAULT_CHAR_WIDTH {
            for j in 0..lfb::DEFAULT_CHAR_HEIGHT {
                display.lfb.lfb().draw_pixel(i, j, color::HHU_GREEN);
            }
        }

        // Collect system information
        let uptime = systime();
        let process_count = process::count();
        let thread_count = thread::count();
        let system_info = system_info();

        // Draw info string
        let info_string = format!(
            "D³OS v{} ({}) | Uptime: {:0>2}:{:0>2}:{:0>2} | Processes: {} | Threads: {}",
            system_info.pkg_version,
            system_info.profile,
            uptime.num_hours(),
            uptime.num_minutes() % 60,
            uptime.num_seconds() - (uptime.num_minutes() * 60),
            process_count,
            thread_count
        );

        display
            .lfb
            .lfb()
            .draw_string(0, 0, color::HHU_BLUE, color::INVISIBLE, info_string.as_str());

        // Draw date
        let date_str = date().format("%Y-%m-%d %H:%M:%S").to_string();

        display.lfb.lfb().draw_string(
            (display.size.0 as u32 - date_str.len() as u32) * lfb::DEFAULT_CHAR_WIDTH,
            0,
            color::HHU_BLUE,
            color::INVISIBLE,
            &date_str,
        );

        display.lfb.flush_lines(0, lfb::DEFAULT_CHAR_HEIGHT);
    }

    fn scroll_up(display: &mut DisplayState, color: &mut ColorState) {
        unsafe {
            let char_ptr = display.char_buffer.as_ptr() as *mut u8;
            char_ptr.copy_from(
                char_ptr.offset(display.size.0 as isize * size_of::<Character>() as isize),
                display.size.0 as usize * (display.size.1 as usize - 1) * size_of::<Character>(),
            );
        }

        let skip = display.size.0 as usize * (display.size.1 as usize - 1);
        display.char_buffer.iter_mut().skip(skip).for_each(|item| {
            item.value = '\0';
            item.fg_color = color.fg_color;
            item.bg_color = color.bg_color;
        });

        let size = display.size;
        display.lfb.lfb().scroll_up(lfb::DEFAULT_CHAR_HEIGHT);
        display.lfb.lfb().fill_rect(
            0,
            (size.1 - 1) as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            size.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        LFBTerminal::draw_status_bar(display);
        display.lfb.flush();
    }

    fn position(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState, pos: (u16, u16)) {
        if pos.1 == 0 {
            cursor.pos = (pos.0, 1);
        } else {
            cursor.pos = pos
        }

        while cursor.pos.1 >= display.size.1 {
            cursor.pos.1 -= 1;
            LFBTerminal::scroll_up(display, color);
        }
    }

    fn handle_bell() {
        // TODO#7 fix speaker access
        // let speaker = speaker();
        // speaker.play(440, 250);
        // speaker.play(880, 250);
    }

    fn handle_tab(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState) {
        if cursor.pos.0 + TAB_SPACES >= display.size.0 {
            LFBTerminal::position(display, cursor, color, (0, cursor.pos.1 + 1));
        } else {
            LFBTerminal::position(
                display,
                cursor,
                color,
                (((cursor.pos.0 + TAB_SPACES) / TAB_SPACES) * TAB_SPACES, cursor.pos.1),
            );
        }
    }

    fn clear_screen(display: &mut DisplayState, color: &mut ColorState) {
        // Clear screen
        let size = display.size;
        display.lfb.lfb().fill_rect(
            0,
            0,
            size.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            size.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear character buffer
        display.char_buffer.iter_mut().for_each(|item| {
            item.value = '\0';
            item.fg_color = color.fg_color;
            item.bg_color = color.bg_color;
        });

        LFBTerminal::draw_status_bar(display);
        display.lfb.flush();
    }

    fn clear_screen_to_cursor(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState) {
        let pos = cursor.pos;
        let size = display.size;

        // Clear from start of line to cursor
        display.lfb.lfb().fill_rect(
            0,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear from start of screen to line before cursor
        display.lfb.lfb().fill_rect(
            0,
            0,
            size.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear character buffer from beginning of screen to cursor
        display
            .char_buffer
            .iter_mut()
            .enumerate()
            .filter(|item| item.0 < (pos.1 * size.0 + pos.0) as usize)
            .for_each(|item| {
                item.1.value = '\0';
                item.1.fg_color = color.fg_color;
                item.1.bg_color = color.bg_color;
            });

        LFBTerminal::draw_status_bar(display);
        display.lfb.flush();
    }

    fn clear_screen_from_cursor(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState) {
        let pos = cursor.pos;
        let size = display.size;

        // Clear from cursor to end of line
        display.lfb.lfb().fill_rect(
            pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            (size.0 - pos.0) as u32 * lfb::DEFAULT_CHAR_WIDTH,
            lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear from next line to end of screen
        display.lfb.lfb().fill_rect(
            0,
            (pos.1 + 1) as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            size.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            (size.1 - pos.1 - 1) as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear character buffer from cursor to end of screen
        display
            .char_buffer
            .iter_mut()
            .skip((pos.1 * size.0 + pos.0) as usize)
            .for_each(|item| {
                item.value = '\0';
                item.fg_color = color.fg_color;
                item.bg_color = color.bg_color;
            });

        LFBTerminal::draw_status_bar(display);
        display.lfb.flush();
    }

    fn clear_line(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState) {
        let pos = cursor.pos;
        let size = display.size;

        // Clear line in lfb
        display.lfb.lfb().fill_rect(
            0,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            size.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );
        // Clear line in character buffer
        display
            .char_buffer
            .iter_mut()
            .skip((pos.1 * size.0) as usize)
            .enumerate()
            .filter(|item| item.0 < size.0 as usize)
            .for_each(|item| {
                item.1.value = ' ';
                item.1.fg_color = color.fg_color;
                item.1.bg_color = color.bg_color;
            });

        if pos.1 == 0 {
            LFBTerminal::draw_status_bar(display);
        }
        display.lfb.flush();
    }

    fn clear_line_to_cursor(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState) {
        let pos = cursor.pos;
        let size = display.size;

        // Clear line in lfb
        display.lfb.lfb().fill_rect(
            0,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear line in character buffer
        display
            .char_buffer
            .iter_mut()
            .skip((pos.1 * size.0) as usize)
            .enumerate()
            .filter(|item| item.0 < pos.0 as usize)
            .for_each(|item| {
                item.1.value = '\0';
                item.1.fg_color = color.fg_color;
                item.1.bg_color = color.bg_color;
            });

        if pos.1 == 0 {
            LFBTerminal::draw_status_bar(display);
        }
        display.lfb.flush();
    }

    fn clear_line_from_cursor(display: &mut DisplayState, cursor: &mut CursorState, color: &mut ColorState) {
        let pos = cursor.pos;
        let size = display.size;

        // Clear line in lfb
        display.lfb.lfb().fill_rect(
            pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            (size.0 - pos.0) as u32 * lfb::DEFAULT_CHAR_WIDTH,
            lfb::DEFAULT_CHAR_HEIGHT,
            color.bg_color,
        );

        // Clear line in character buffer
        display
            .char_buffer
            .iter_mut()
            .skip((pos.1 * size.0 + pos.0) as usize)
            .enumerate()
            .filter(|item| item.0 < (size.0 - pos.0) as usize)
            .for_each(|item| {
                item.1.value = '\0';
                item.1.fg_color = color.fg_color;
                item.1.bg_color = color.bg_color;
            });

        if pos.1 == 0 {
            LFBTerminal::draw_status_bar(display);
        }
        display.lfb.flush();
    }

    fn handle_ansi_color(color: &mut ColorState, params: &Params) {
        let mut iter = params.iter();
        while let Some(param) = iter.next() {
            let code = param[0];

            match code {
                0..=29 => {
                    LFBTerminal::handle_ansi_graphic_rendition(color, code);
                }
                30..=39 => {
                    if let Some(col) = ansi_color(code - 30, &mut iter) {
                        color.fg_base_color = col;
                        color.fg_bright = false;
                    }
                }
                40..=49 => {
                    if let Some(col) = ansi_color(code - 40, &mut iter) {
                        color.bg_base_color = col;
                        color.bg_bright = false;
                    }
                }
                90..=97 => {
                    if let Some(col) = ansi_color(code - 90, &mut iter) {
                        color.fg_base_color = col;
                        color.fg_bright = true;
                    }
                }
                100..=107 => {
                    if let Some(col) = ansi_color(code - 100, &mut iter) {
                        color.bg_base_color = col;
                        color.bg_bright = true;
                    }
                }
                _ => {}
            }
        }

        let mut fg_self = color.fg_base_color;
        let mut bg_self = color.bg_base_color;

        if color.invert {
            core::mem::swap(&mut fg_self, &mut bg_self);
        }

        if color.bright || color.fg_bright {
            fg_self = fg_self.bright();
        }

        if color.dim {
            fg_self = fg_self.dim();
        }

        if color.bg_bright {
            bg_self = bg_self.bright();
        }

        color.fg_color = fg_self;
        color.bg_color = bg_self;
    }

    fn handle_ansi_graphic_rendition(color: &mut ColorState, code: u16) {
        match code {
            0 => {
                color.fg_base_color = color::WHITE;
                color.bg_base_color = color::BLACK;
                color.fg_color = color::WHITE;
                color.bg_color = color::BLACK;
                color.fg_bright = false;
                color.bg_bright = false;
                color.invert = false;
                color.bright = false;
                color.dim = false;
            }
            1 => {
                color.bright = true;
            }
            2 => {
                color.dim = true;
            }
            7 => {
                color.invert = true;
            }
            22 => {
                color.bright = false;
                color.dim = false;
            }
            27 => {
                color.invert = false;
            }
            _ => {}
        }
    }

    fn handle_ansi_cursor_sequence(
        display: &mut DisplayState,
        cursor: &mut CursorState,
        color: &mut ColorState,
        code: u8,
        params: &Params,
    ) {
        let mut iter = params.iter();
        match code {
            0x41 => {
                // Move cursor up
                let param = iter.next();
                if let Some(p) = param {
                    let y_move = p[0];
                    let row = cursor.pos.1 - if y_move == 0 { 1 } else { y_move };
                    LFBTerminal::position(display, cursor, color, (cursor.pos.0, if row > 0 { row } else { 0 }));
                }
            }
            0x42 => {
                // Move cursor down
                let param = iter.next();
                if let Some(p) = param {
                    let y_move = p[0];
                    let row = cursor.pos.1 + if y_move == 0 { 1 } else { y_move };
                    LFBTerminal::position(
                        display,
                        cursor,
                        color,
                        (
                            cursor.pos.0,
                            if row < display.size.1 { row } else { display.size.1 - 1 },
                        ),
                    );
                };
            }
            0x43 => {
                // Move cursor right
                let param = iter.next();
                if let Some(p) = param {
                    let x_move = p[0];
                    let column = cursor.pos.0 + if x_move == 0 { 1 } else { x_move };
                    LFBTerminal::position(
                        display,
                        cursor,
                        color,
                        (
                            if column < display.size.0 {
                                column
                            } else {
                                display.size.0 - 1
                            },
                            cursor.pos.1,
                        ),
                    );
                };
            }
            0x44 => {
                // Move cursor left
                let param = iter.next();
                if let Some(p) = param {
                    let x_move = p[0];
                    let column = cursor.pos.0 - if x_move == 0 { 1 } else { x_move };
                    LFBTerminal::position(
                        display,
                        cursor,
                        color,
                        (if column > 0 { column } else { 0 }, cursor.pos.1),
                    );
                };
            }
            0x45 => {
                // Move cursor to start of next line
                let param = iter.next();
                if let Some(p) = param {
                    let row = cursor.pos.1 + p[0] + 1;
                    LFBTerminal::position(
                        display,
                        cursor,
                        color,
                        (0, if row < display.size.1 { row } else { display.size.1 - 1 }),
                    );
                };
            }
            0x46 => {
                // Move cursor to start of previous line
                let param = iter.next();
                if let Some(p) = param {
                    let row = cursor.pos.1 - p[0] - 1;
                    LFBTerminal::position(display, cursor, color, (0, if row > 0 { row } else { 0 }));
                };
            }
            0x47 => {
                // Move cursor to column
                let param = iter.next();
                if let Some(p) = param {
                    let column = p[0];
                    LFBTerminal::position(
                        display,
                        cursor,
                        color,
                        (
                            if column < display.size.0 {
                                column
                            } else {
                                display.size.0 - 1
                            },
                            cursor.pos.1,
                        ),
                    );
                }
            }
            0x48 | 0x66 => {
                // Set cursor position
                let param1 = iter.next();
                let param2 = iter.next();

                if let Some(p1) = param1 && let Some(p2) = param2 {
                    let column = p1[0];
                    let row = p2[0];

                    LFBTerminal::position(
                        display,
                        cursor,
                        color,
                        (
                            if column > display.size.0 {
                                display.size.0 - 1
                            } else {
                                column
                            },
                            if row > display.size.1 { display.size.1 - 1 } else { row },
                        ),
                    );
                } else {
                    LFBTerminal::position(display, cursor, color, (0, 0));
                }
            }
            0x73 => {
                // Save cursor position
                cursor.saved_pos = (cursor.pos.0, cursor.pos.1);
            }
            0x75 => {
                // Restore cursor position
                LFBTerminal::position(display, cursor, color, cursor.saved_pos);
            }
            _ => {}
        }
        display.lfb.flush(); // Fixes trailing cursor
    }

    fn handle_ansi_erase_sequence(
        display: &mut DisplayState,
        cursor: &mut CursorState,
        color: &mut ColorState,
        code: u8,
        params: &Params,
    ) {
        let mut iter = params.iter();
        let param = iter.next();
        let erase_code = if let Some(p) = param { p[0] } else { 0 };

        match code {
            0x4a => match erase_code {
                0 => LFBTerminal::clear_screen_from_cursor(display, cursor, color),
                1 => LFBTerminal::clear_screen_to_cursor(display, cursor, color),
                2 => {
                    LFBTerminal::clear_screen(display, color);
                    LFBTerminal::position(display, cursor, color, (0, 0));
                }
                _ => {}
            },
            0x4b => match erase_code {
                0 => LFBTerminal::clear_line_from_cursor(display, cursor, color),
                1 => LFBTerminal::clear_line_to_cursor(display, cursor, color),
                2 => LFBTerminal::clear_line(display, cursor, color),
                _ => {}
            },
            _ => {}
        }
    }
}

fn ansi_color(code: u16, iter: &mut ParamsIter) -> Option<Color> {
    match code {
        0 => Some(color::BLACK),
        1 => Some(color::RED),
        2 => Some(color::GREEN),
        3 => Some(color::YELLOW),
        4 => Some(color::BLUE),
        5 => Some(color::MAGENTA),
        6 => Some(color::CYAN),
        7 | 9 => Some(color::WHITE),
        8 => parse_complex_color(iter),
        _ => None,
    }
}

fn parse_complex_color(iter: &mut ParamsIter) -> Option<Color> {
    let mode = iter.next()?[0];

    match mode {
        2 => {
            let red = iter.next()?[0] as u8;
            let green = iter.next()?[0] as u8;
            let blue = iter.next()?[0] as u8;

            Some(Color {
                red,
                green,
                blue,
                alpha: 255,
            })
        }
        5 => {
            let index = iter.next()?[0] as usize;
            if index <= 255 {
                Some(COLOR_TABLE_256[index])
            } else {
                None
            }
        }
        _ => None,
    }
}

impl Perform for LFBTerminal {
    fn print(&mut self, c: char) {
        self.print_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => LFBTerminal::handle_bell(),
            0x09 => LFBTerminal::handle_tab(
                &mut self.display.lock(),
                &mut self.cursor.lock(),
                &mut self.color.lock(),
            ),
            0x0a => self.print_char('\n'),
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: u8) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: u8) {
        match action {
            0x41..=0x48 | 0x66 | 0x6e | 0x73 | 0x75 => LFBTerminal::handle_ansi_cursor_sequence(
                &mut self.display.lock(),
                &mut self.cursor.lock(),
                &mut self.color.lock(),
                action,
                params,
            ),
            0x4a | 0x4b => LFBTerminal::handle_ansi_erase_sequence(
                &mut self.display.lock(),
                &mut self.cursor.lock(),
                &mut self.color.lock(),
                action,
                params,
            ),
            0x6d => LFBTerminal::handle_ansi_color(&mut self.color.lock(), params),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}
