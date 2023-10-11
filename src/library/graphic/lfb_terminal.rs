use alloc::vec::Vec;
use core::{fmt};
use core::fmt::Write;
use core::mem::size_of;
use anstyle_parse::{Params, ParamsIter, Parser, Perform, Utf8Parser};
use spin::Mutex;
use crate::device::speaker;
use crate::library::graphic::{color, lfb};
use crate::library::graphic::ansi::COLOR_TABLE_256;
use crate::library::graphic::buffered_lfb::BufferedLFB;
use crate::library::graphic::color::Color;
use crate::library::graphic::lfb::LFB;

// The global writer that can be used as an interface from other modules
// It is thread safe by using 'Mutex'
pub static mut WRITER: Mutex<Terminal> = Mutex::new(Terminal::empty());

pub fn initialize(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) {
    unsafe { WRITER = Mutex::new(Terminal::new(buffer, pitch, width, height, bpp, true)); }
}

const CURSOR: char = if let Some(cursor) = char::from_u32(0x2588) { cursor } else { '_' };
const TAB_SPACES: u32 = 8;

pub struct Terminal {
    lfb: BufferedLFB,
    char_buffer: Vec<Character>,
    parser: Option<Parser>,

    columns: u32,
    rows: u32,

    x: u32,
    y: u32,

    fg_color: Color,
    bg_color: Color,
    fg_base_color: Color,
    bg_base_color: Color,
    fg_bright: bool,
    bg_bright: bool,
    invert: bool,
    bright: bool,
    dim: bool,

    ansi_saved_x: u32,
    ansi_saved_y: u32
}

struct Character {
    value: char,
    fg_color: Color,
    bg_color: Color
}

impl Terminal {
    pub const fn empty() -> Self {
        Self { lfb: BufferedLFB::empty(), char_buffer: Vec::new(), parser: None, columns: 0, rows: 0, x: 0, y: 0, fg_color: color::INVISIBLE, bg_color: color::INVISIBLE, fg_base_color: color::INVISIBLE, bg_base_color: color::INVISIBLE, fg_bright: false, bg_bright: false, invert: false, bright: false, dim: false, ansi_saved_x: 0, ansi_saved_y: 0 }
    }

    pub fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8, ansi_support: bool) -> Self {
        let raw_lfb = LFB::new(buffer, pitch, width, height, bpp);
        let mut lfb = BufferedLFB::new(raw_lfb);

        lfb.lfb().clear();
        lfb.lfb().draw_char(0, 0, color::WHITE, color::BLACK, CURSOR);
        lfb.flush();

        let columns = width / lfb::CHAR_WIDTH;
        let rows = height / lfb::CHAR_HEIGHT;
        let size = columns as usize * rows as usize * size_of::<Character>();

        let mut char_buffer = Vec::with_capacity(size);
        for _ in 0..size {
            char_buffer.push(Character { value: ' ', fg_color: color::WHITE, bg_color: color::BLACK });
        }

        let parser = if ansi_support { Some(Parser::<Utf8Parser>::new()) } else { None };

        Self { lfb, char_buffer, parser, columns, rows, x: 0, y: 0, fg_color: color::WHITE, bg_color: color::BLACK, fg_base_color: color::WHITE, bg_base_color: color::BLACK, fg_bright: false, bg_bright: false, invert: false, bright: false, dim: false, ansi_saved_x: 0, ansi_saved_y: 0 }
    }

    fn print_char_at(&mut self, c: char, x: u32, y: u32, fg_color: Color, bg_color: Color) -> bool {
        self.lfb.lfb().draw_char(x * lfb::CHAR_WIDTH, y * lfb::CHAR_HEIGHT, fg_color, bg_color, c) &&
            self.lfb.direct_lfb().draw_char(x * lfb::CHAR_WIDTH, y * lfb::CHAR_HEIGHT, fg_color, bg_color, c)
    }

    fn print_char(&mut self, c: char, fg_color: Color, bg_color: Color) {
        if c == '\n' {
            self.clear_cursor();
            self.clear_line_from_cursor();

            self.y += 1;
            self.x = 0;
        } else {
            if self.print_char_at(c, self.x, self.y, fg_color, bg_color) {
                let index = (self.y * self.columns + self.x) as usize;
                self.char_buffer[index] = Character { value: c, fg_color, bg_color };

                self.x += 1;
            }
        }

        if self.x >= self.columns {
            self.y += 1;
            self.x = 0;
        }

        if self.y >= self.rows {
            self.scroll_up();
            self.x = 0;
            self.y = self.rows - 1;
        }

        self.print_cursor();
    }

    fn write_char(&mut self, c: char) {
        self.print_char(c, self.fg_color, self.bg_color);
    }

    pub fn write_byte(&mut self, c: u8) {
        if self.parser.is_some() {
            let mut parser = self.parser.as_mut().unwrap().clone();
            parser.advance(self, c);
            self.parser = Some(parser);
        } else {
            self.write_char(char::from(c));
        }
    }

    fn clear_cursor(&mut self) {
        let character: &Character = &self.char_buffer[(self.y * self.columns + self.x) as usize];
        self.print_char_at(character.value, self.x, self.y, character.fg_color, character.bg_color);
    }

    fn print_cursor(&mut self) {
        self.print_char_at(CURSOR, self.x, self.y, self.fg_color, self.bg_color);
    }

    fn scroll_up(&mut self) {
        unsafe {
            let char_ptr = self.char_buffer.as_ptr() as *mut u8;
            char_ptr.copy_from(char_ptr.offset(self.columns as isize * size_of::<Character>() as isize), self.columns as usize * (self.rows as usize - 1) * size_of::<Character>());
        }

        self.char_buffer.iter_mut().skip(self.columns as usize * (self.rows as usize - 1)).for_each(|item| {
            item.value = '\0';
            item.fg_color = self.fg_color;
            item.bg_color = self.bg_color;
        });

        self.lfb.lfb().scroll_up(lfb::CHAR_HEIGHT);
        self.lfb.lfb().fill_rect(0, (self.rows - 1) * lfb::CHAR_HEIGHT, self.columns * lfb::CHAR_WIDTH, lfb::CHAR_HEIGHT, self.bg_color);
        self.lfb.flush();
    }

    fn set_pos(&mut self, x: u32, y: u32) {
        self.clear_cursor();
        self.x = x;
        self.y = y;

        while self.y >= self.rows {
            self.scroll_up();
            self.y -= 1;
        }

        self.print_cursor();
    }

    fn handle_bell(&self) {
        let mut speaker = speaker::get_speaker().lock();
        speaker.play(440, 250);
        speaker.play(880, 250);
    }

    fn handle_tab(&mut self) {
        if self.x + TAB_SPACES >= self.columns {
            self.set_pos(0, self.y + 1);
        } else {
            self.set_pos(((self.x + TAB_SPACES) / TAB_SPACES) * TAB_SPACES, self.y);
        }
    }

    fn clear_screen(&mut self) {
        // Clear screen
        self.lfb.lfb().fill_rect(0, 0, self.columns * lfb::CHAR_WIDTH, self.rows * lfb::CHAR_HEIGHT, self.bg_color);
        // Clear character buffer
        self.char_buffer.iter_mut()
            .for_each(|item| {
                item.value = '\0';
                item.fg_color = self.fg_color;
                item.bg_color = self.bg_color;
            });

        self.lfb.flush();
    }

    fn clear_screen_to_cursor(&mut self) {
        // Clear from start of line to cursor
        self.lfb.lfb().fill_rect(0, self.y * lfb::CHAR_HEIGHT, self.x * lfb::CHAR_WIDTH, lfb::CHAR_HEIGHT, self.bg_color);
        // Clear from start of screen to line before cursor
        self.lfb.lfb().fill_rect(0, 0, self.columns * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, self.bg_color);
        // Clear character buffer from beginning of screen to cursor
        self.char_buffer.iter_mut().enumerate()
            .filter(|item| {
                item.0 < (self.y * self.columns + self.x) as usize
            })
            .for_each(|item| {
                item.1.value = '\0';
                item.1.fg_color = self.fg_color;
                item.1.bg_color = self.bg_color;
            });

        self.lfb.flush();
    }

    fn clear_screen_from_cursor(&mut self) {
        // Clear from cursor to end of line
        self.lfb.lfb().fill_rect(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, (self.columns - self.x) * lfb::CHAR_WIDTH, lfb::CHAR_HEIGHT, self.bg_color);
        // Clear from next line to end of screen
        self.lfb.lfb().fill_rect(0, (self.y + 1) * lfb::CHAR_HEIGHT, self.columns * lfb::CHAR_WIDTH, (self.rows - self.y - 1) * lfb::CHAR_HEIGHT, self.bg_color);
        // Clear character buffer from cursor to end of screen
        self.char_buffer.iter_mut().skip((self.y * self.columns + self.x) as usize)
            .for_each(|item| {
                item.value = '\0';
                item.fg_color = self.fg_color;
                item.bg_color = self.bg_color;
            });

        self.lfb.flush();
    }

    fn clear_line(&mut self) {
        // Clear line in lfb
        self.lfb.lfb().fill_rect(0, self.y * lfb::CHAR_HEIGHT, self.columns * lfb::CHAR_WIDTH, lfb::CHAR_HEIGHT, self.bg_color);
        // Clear line in character buffer
        self.char_buffer.iter_mut().skip((self.y * self.columns) as usize).enumerate()
            .filter(|item| {
                item.0 < self.columns as usize
            })
            .for_each(|item| {
                item.1.value = 'a';
                item.1.fg_color = self.fg_color;
                item.1.bg_color = self.bg_color;
            });

        self.lfb.flush();
    }

    fn clear_line_to_cursor(&mut self) {
        // Clear line in lfb
        self.lfb.lfb().fill_rect(0, self.y * lfb::CHAR_HEIGHT, self.x * lfb::CHAR_WIDTH, lfb::CHAR_HEIGHT, self.bg_color);
        // Clear line in character buffer
        self.char_buffer.iter_mut().skip((self.y * self.columns) as usize).enumerate()
            .filter(|item| {
                item.0 < self.x as usize
            })
            .for_each(|item| {
                item.1.value = '\0';
                item.1.fg_color = self.fg_color;
                item.1.bg_color = self.bg_color;
            });

        self.lfb.flush();
    }

    fn clear_line_from_cursor(&mut self) {
        // Clear line in lfb
        self.lfb.lfb().fill_rect(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, (self.columns - self.x) * lfb::CHAR_WIDTH, lfb::CHAR_HEIGHT, self.bg_color);
        // Clear line in character buffer
        self.char_buffer.iter_mut().skip((self.y * self.columns + self.x) as usize).enumerate()
            .filter(|item| {
                item.0 < (self.columns - self.x) as usize
            })
            .for_each(|item| {
                item.1.value = '\0';
                item.1.fg_color = self.fg_color;
                item.1.bg_color = self.bg_color;
            });

        self.lfb.flush();
    }

    fn handle_ansi_color(&mut self, params: &Params) {
        let mut iter = params.iter();
        while let Some(param) = iter.next() {
            let code = param[0];

            match code {
                0..=29 => {
                    self.handle_ansi_graphic_rendition(code);
                },
                30..=39 => {
                    if let Some(color) = get_color(code - 30, &mut iter) {
                        self.fg_base_color = color;
                        self.fg_bright = false;
                    }
                },
                40..=49 => {
                    if let Some(color) = get_color(code - 40, &mut iter) {
                        self.bg_base_color = color;
                        self.bg_bright = false;
                    }
                },
                90..=97 => {
                    if let Some(color) = get_color(code - 90, &mut iter) {
                        self.fg_base_color = color;
                        self.fg_bright = true;
                    }
                },
                100..=107 => {
                    if let Some(color) = get_color(code - 100, &mut iter) {
                        self.bg_base_color = color;
                        self.bg_bright = true;
                    }
                }
                _ => {}
            }
        }

        let mut fg_color = self.fg_base_color;
        let mut bg_color = self.bg_base_color;

        if self.invert {
            let tmp = fg_color;
            fg_color = bg_color;
            bg_color = tmp;
        }

        if self.bright || self.fg_bright {
            fg_color = fg_color.bright();
        }

        if self.dim {
            fg_color = fg_color.dim();
        }

        if self.bg_bright {
            bg_color = bg_color.bright();
        }

        self.fg_color = fg_color;
        self.bg_color = bg_color;
    }

    fn handle_ansi_graphic_rendition(&mut self, code: u16) {
        match code {
            0 => {
                self.fg_base_color = color::WHITE;
                self.bg_base_color = color::BLACK;
                self.fg_color = color::WHITE;
                self.bg_color = color::BLACK;
                self.fg_bright = false;
                self.bg_bright = false;
                self.invert = false;
                self.bright = false;
                self.dim = false;
            },
            1 => {
                self.bright = true;
            },
            2 => {
                self.dim = true;
            },
            7 => {
                self.invert = true;
            },
            22 => {
                self.bright = false;
                self.dim = false;
            },
            27 => {
                self.invert = false;
            }
            _ => {}
        }
    }

    fn handle_ansi_cursor_sequence(&mut self, code: u8, params: &Params) {
        let mut iter = params.iter();
        match code {
            0x41 => { // Move cursor up
                let param = iter.next();
                if param.is_some() {
                    let y_move = param.unwrap()[0] as i32;
                    let row = self.y as i32 - if y_move == 0 { 1 } else { y_move };
                    self.set_pos(self.x, if row > 0 { row as u32 } else { 0 });
                }
            },
            0x42 => { // Move cursor down
                let param = iter.next();
                if param.is_some() {
                    let y_move = param.unwrap()[0] as u32;
                    let row = self.y + if y_move == 0 { 1 } else { y_move };
                    self.set_pos(self.x, if row < self.rows { row } else { self.rows - 1 });
                };
            },
            0x43 => { // Move cursor right
                let param = iter.next();
                if param.is_some() {
                    let x_move = param.unwrap()[0] as u32;
                    let column = self.x + if x_move == 0 { 1 } else { x_move };
                    self.set_pos(if column < self.columns { column } else { self.columns - 1 }, self.y);
                };
            },
            0x44 => { // Move cursor left
                let param = iter.next();
                if param.is_some() {
                    let x_move = param.unwrap()[0] as i32;
                    let column = self.x as i32 - if x_move == 0 { 1 } else { x_move };
                    self.set_pos(if column > 0 { column as u32 } else { 0 }, self.y);
                };
            },
            0x45 => { // Move cursor to start of next line
                let param = iter.next();
                if param.is_some() {
                    let row = self.y + param.unwrap()[0] as u32 + 1;
                    self.set_pos(0, if row < self.rows { row } else { self.rows - 1 });
                };
            },
            0x46 => { // Move cursor to start of previous line
                let param = iter.next();
                if param.is_some() {
                    let row = self.y as i32 - param.unwrap()[0] as i32 - 1;
                    self.set_pos(0, if row > 0 { row as u32 } else { 0 });
                };
            }
            0x47 => { // Move cursor to column
                let param = iter.next();
                if param.is_some() {
                    let column = param.unwrap()[0] as u32;
                    self.set_pos(if column < self.columns { column } else { self.columns - 1 }, self.y)
                }
            }
            0x48 | 0x66 => { // Set cursor position
                let param1 = iter.next();
                let param2 = iter.next();

                if param1.is_some() && param2.is_some() {
                    let column = param1.unwrap()[0] as u32;
                    let row = param2.unwrap()[0] as u32;

                    self.set_pos(if column > self.columns { self.columns - 1 } else { column }, if row > self.rows { self.rows - 1 } else { row });
                } else {
                    self.set_pos(0, 0);
                }
            }
            0x73 => { // Save cursor position
                self.ansi_saved_x = self.x;
                self.ansi_saved_y = self.y;
            }
            0x75 => { // Restore cursor position
                self.set_pos(self.ansi_saved_x, self.ansi_saved_y);
            }
            _ => {}
        }
    }

    fn handle_ansi_erase_sequence(&mut self, code: u8, params: &Params) {
        let mut iter = params.iter();
        let param = iter.next();
        let erase_code = if param.is_some() { param.unwrap()[0] } else { 0 };

        match code {
            0x4a => {
                match erase_code {
                    0 => self.clear_screen_from_cursor(),
                    1 => self.clear_screen_to_cursor(),
                    2 => {
                        self.clear_screen();
                        self.set_pos(0, 0);
                    }
                    _ => {}
                }
            },
            0x4b => {
                match erase_code {
                    0 => self.clear_line_from_cursor(),
                    1 => self.clear_line_to_cursor(),
                    2 => self.clear_line(),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn get_color(code: u16, iter: &mut ParamsIter) -> Option<Color> {
    match code {
        0 => Some(color::BLACK),
        1 => Some(color::RED),
        2 => Some(color::GREEN),
        3 => Some(color::YELLOW),
        4 => Some(color::BLUE),
        5 => Some(color::MAGENTA),
        6 => Some(color::CYAN),
        7 | 9 => Some(color::WHITE),
        8 => {
            parse_complex_color(iter)
        }
        _ => None
    }
}

fn parse_complex_color(iter: &mut ParamsIter) -> Option<Color> {
    let mode = iter.next()?[0];

    return match mode {
        2 => {
            let red = iter.next()?[0] as u8;
            let green = iter.next()?[0] as u8;
            let blue = iter.next()?[0] as u8;

            Some(Color { red, green, blue, alpha: 255 })
        }
        5 => {
            let index = iter.next()?[0] as usize;
            if index <= 255 { Some(COLOR_TABLE_256[index]) } else { None }
        }
        _ => {
            None
        }
    }
}

impl Perform for Terminal {
    fn print(&mut self, c: char) {
        self.write_char(c);
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            0x07 => self.handle_bell(),
            0x09 => self.handle_tab(),
            0x0a => self.write_char('\n'),
            _ => {}
        }
    }

    fn hook(&mut self, _params: &Params, _intermediates: &[u8], _ignore: bool, _action: u8) {}

    fn put(&mut self, _byte: u8) {}

    fn unhook(&mut self) {}

    fn osc_dispatch(&mut self, _params: &[&[u8]], _bell_terminated: bool) {}

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], _ignore: bool, action: u8,) {
        match action {
            0x41..=0x48 | 0x66 | 0x6e | 0x73 | 0x75 => self.handle_ansi_cursor_sequence(action, params),
            0x4a | 0x4b => self.handle_ansi_erase_sequence(action, params),
            0x6d => self.handle_ansi_color(params),
            _ => {}
        }
    }

    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {}
}

// Implementation of the 'core::fmt::Write' trait for our Terminal
// Required to output formatted strings
// Requires only one function 'write_str'
impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            self.write_byte(c);
        }

        Ok(())
    }
}

// Provide macros like in the 'io' module of Rust
// The $crate variable ensures that the macro also works
// from outside the 'std' crate.
macro_rules! print {
    ($($arg:tt)*) => ({
        $crate::lfb_terminal::print(format_args!($($arg)*));
    });
}

macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

// Helper function of print macros (must be public)
pub fn print(args: fmt::Arguments) {
    unsafe { WRITER.lock().write_fmt(args).unwrap() };
}