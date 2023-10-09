use core::fmt;
use core::fmt::Write;
use spin::Mutex;
use crate::library::graphic::{color, lfb};
use crate::library::graphic::buffered_lfb::BufferedLFB;
use crate::library::graphic::color::Color;
use crate::library::graphic::lfb::LFB;

// The global writer that can used as an interface from other modules
// It is thread safe by using 'Mutex'
static mut WRITER: Mutex<Terminal> = Mutex::new(Terminal::empty());

pub fn initialize(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) {
    unsafe { WRITER = Mutex::new(Terminal::new(buffer, pitch, width, height, bpp)); }
}

pub fn get_writer() -> &'static Mutex<Terminal> {
    unsafe { &WRITER }
}

const CURSOR: char = if let Some(cursor) = char::from_u32(0x2588) { cursor } else { '_' };

pub struct Terminal {
    lfb: BufferedLFB,
    columns: u32,
    rows: u32,
    x: u32,
    y: u32
}

impl Terminal {
    pub const fn empty() -> Self {
        Self { lfb: BufferedLFB::empty(), columns: 0, rows: 0, x: 0, y: 0 }
    }

    pub fn new(buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let raw_lfb = LFB::new(buffer, pitch, width, height, bpp);
        let mut lfb = BufferedLFB::new(raw_lfb);

        lfb.lfb().clear();
        lfb.lfb().draw_char(0, 0, &color::WHITE, &color::BLACK, CURSOR);
        lfb.flush();

        Self { lfb, columns: width / lfb::CHAR_WIDTH, rows: height / lfb::CHAR_HEIGHT, x: 0, y: 0 }
    }

    pub fn print_char(&mut self, c: char, fg_color: &Color, bg_color: &Color) {
        if c == '\n' {
            // Clear cursor
            self.lfb.lfb().draw_char(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, &color::INVISIBLE, bg_color, ' ');
            self.lfb.direct_lfb().draw_char(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, &color::INVISIBLE, bg_color, ' ');

            self.y += 1;
            self.x = 0;
        } else {
            if self.lfb.lfb().draw_char(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, fg_color, bg_color, c) {
                self.lfb.direct_lfb().draw_char(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, fg_color, bg_color, c);
                self.x += 1;
            }
        }

        if self.x >= self.columns {
            self.y += 1;
            self.x = 0;
        }

        if self.y >= self.rows {
            self.lfb.lfb().scroll_up(lfb::CHAR_HEIGHT);
            self.lfb.flush();
            self.x = 0;
            self.y = self.rows - 1;
        }

        // Draw cursor
        self.lfb.lfb().draw_char(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, fg_color, bg_color, CURSOR);
        self.lfb.direct_lfb().draw_char(self.x * lfb::CHAR_WIDTH, self.y * lfb::CHAR_HEIGHT, fg_color, bg_color, CURSOR);
    }
}

// Implementation of the 'core::fmt::Write' trait for our Terminal
// Required to output formatted strings
// Requires only one function 'write_str'
impl Write for Terminal {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.print_char(c, &color::WHITE, &color::BLACK);
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
    get_writer().lock().write_fmt(args).unwrap();
}