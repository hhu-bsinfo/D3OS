use alloc::format;
use alloc::string::String;
use crate::library::graphic::color;
use crate::library::graphic::color::Color;

pub const ESCAPE_SEQUENCE_START: char = '\x1b';
pub const RESET: &str = "\x1b[0m";
pub const FOREGROUND_BLACK: &str = "\x1b[30m";
pub const FOREGROUND_RED: &str = "\x1b[31m";
pub const FOREGROUND_GREEN: &str = "\x1b[32m";
pub const FOREGROUND_YELLOW: &str = "\x1b[33m";
pub const FOREGROUND_BLUE: &str = "\x1b[34m";
pub const FOREGROUND_MAGENTA: &str = "\x1b[35m";
pub const FOREGROUND_CYAN: &str = "\x1b[36m";
pub const FOREGROUND_WHITE: &str = "\x1b[37m";
pub const FOREGROUND_DEFAULT: &str = "\x1b[39m";
pub const FOREGROUND_BRIGHT_BLACK: &str = "\x1b[90m";
pub const FOREGROUND_BRIGHT_RED: &str = "\x1b[91m";
pub const FOREGROUND_BRIGHT_GREEN: &str = "\x1b[92m";
pub const FOREGROUND_BRIGHT_YELLOW: &str = "\x1b[93m";
pub const FOREGROUND_BRIGHT_BLUE: &str = "\x1b[94m";
pub const FOREGROUND_BRIGHT_MAGENTA: &str = "\x1b[95m";
pub const FOREGROUND_BRIGHT_CYAN: &str = "\x1b[96m";
pub const FOREGROUND_BRIGHT_WHITE: &str = "\x1b[97m";
pub const BACKGROUND_BLACK: &str = "\x1b[40m";
pub const BACKGROUND_RED: &str = "\x1b[41m";
pub const BACKGROUND_GREEN: &str = "\x1b[42m";
pub const BACKGROUND_YELLOW: &str = "\x1b[43m";
pub const BACKGROUND_BLUE: &str = "\x1b[44m";
pub const BACKGROUND_MAGENTA: &str = "\x1b[45m";
pub const BACKGROUND_CYAN: &str = "\x1b[46m";
pub const BACKGROUND_WHITE: &str = "\x1b[47m";
pub const BACKGROUND_DEFAULT: &str = "\x1b[49m";
pub const BACKGROUND_BRIGHT_BLACK: &str = "\x1b[100m";
pub const BACKGROUND_BRIGHT_RED: &str = "\x1b[101m";
pub const BACKGROUND_BRIGHT_GREEN: &str = "\x1b[102m";
pub const BACKGROUND_BRIGHT_YELLOW: &str = "\x1b[103m";
pub const BACKGROUND_BRIGHT_BLUE: &str = "\x1b[104m";
pub const BACKGROUND_BRIGHT_MAGENTA: &str = "\x1b[105m";
pub const BACKGROUND_BRIGHT_CYAN: &str = "\x1b[106m";
pub const BACKGROUND_BRIGHT_WHITE: &str = "\x1b[107m";

pub const COLOR_TABLE_256: [Color;256] = [
    // 16 predefined colors, matching the 4-bit ANSI colors
    color::BLACK, color::RED, color::GREEN, color::YELLOW, color::BLUE, color::MAGENTA, color::CYAN, color::WHITE,
    color::BLACK.bright(), color::RED.bright(), color::GREEN.bright(), color::YELLOW.bright(), color::BLUE.bright(), color::MAGENTA.bright(), color::CYAN.bright(), color::WHITE.bright(),

    // 216 colors
    Color { red: 0, green: 0, blue: 0, alpha: 255 }, Color { red: 0, green: 0, blue: 95, alpha: 255 }, Color { red: 0, green: 0, blue: 135, alpha: 255 },
    Color { red: 0, green: 0, blue: 175, alpha: 255 }, Color { red: 0, green: 0, blue: 215, alpha: 255 }, Color { red: 0, green: 0, blue: 255, alpha: 255 },

    Color { red: 0, green: 95, blue: 0, alpha: 255 }, Color { red: 0, green: 95, blue: 95, alpha: 255 }, Color { red: 0, green: 95, blue: 135, alpha: 255 },
    Color { red: 0, green: 95, blue: 175, alpha: 255 }, Color { red: 0, green: 95, blue: 215, alpha: 255 }, Color { red: 0, green: 95, blue: 255, alpha: 255 },

    Color { red: 0, green: 135, blue: 0, alpha: 255 }, Color { red: 0, green: 135, blue: 95, alpha: 255 }, Color { red: 0, green: 135, blue: 135, alpha: 255 },
    Color { red: 0, green: 135, blue: 175, alpha: 255 }, Color { red: 0, green: 135, blue: 215, alpha: 255 }, Color { red: 0, green: 135, blue: 255, alpha: 255 },

    Color { red: 0, green: 175, blue: 0, alpha: 255 }, Color { red: 0, green: 175, blue: 95, alpha: 255 }, Color { red: 0, green: 175, blue: 135, alpha: 255 },
    Color { red: 0, green: 175, blue: 175, alpha: 255 }, Color { red: 0, green: 175, blue: 215, alpha: 255 }, Color { red: 0, green: 175, blue: 255, alpha: 255 },

    Color { red: 0, green: 215, blue: 0, alpha: 255 }, Color { red: 0, green: 215, blue: 95, alpha: 255 }, Color { red: 0, green: 215, blue: 135, alpha: 255 },
    Color { red: 0, green: 215, blue: 175, alpha: 255 }, Color { red: 0, green: 215, blue: 215, alpha: 255 }, Color { red: 0, green: 215, blue: 255, alpha: 255 },

    Color { red: 0, green: 255, blue: 0, alpha: 255 }, Color { red: 0, green: 255, blue: 95, alpha: 255 }, Color { red: 0, green: 255, blue: 135, alpha: 255 },
    Color { red: 0, green: 255, blue: 175, alpha: 255 }, Color { red: 0, green: 255, blue: 215, alpha: 255 }, Color { red: 0, green: 255, blue: 255, alpha: 255 },

    Color { red: 95, green: 0, blue: 0, alpha: 255 }, Color { red: 95, green: 0, blue: 95, alpha: 255 }, Color { red: 95, green: 0, blue: 135, alpha: 255 },
    Color { red: 95, green: 0, blue: 175, alpha: 255 }, Color { red: 95, green: 0, blue: 215, alpha: 255 }, Color { red: 95, green: 0, blue: 255, alpha: 255 },

    Color { red: 95, green: 95, blue: 0, alpha: 255 }, Color { red: 95, green: 95, blue: 95, alpha: 255 }, Color { red: 95, green: 95, blue: 135, alpha: 255 },
    Color { red: 95, green: 95, blue: 175, alpha: 255 }, Color { red: 95, green: 95, blue: 215, alpha: 255 }, Color { red: 95, green: 95, blue: 255, alpha: 255 },

    Color { red: 95, green: 135, blue: 0, alpha: 255 }, Color { red: 95, green: 135, blue: 95, alpha: 255 }, Color { red: 95, green: 135, blue: 135, alpha: 255 },
    Color { red: 95, green: 135, blue: 175, alpha: 255 }, Color { red: 95, green: 135, blue: 215, alpha: 255 }, Color { red: 95, green: 135, blue: 255, alpha: 255 },

    Color { red: 95, green: 175, blue: 0, alpha: 255 }, Color { red: 95, green: 175, blue: 95, alpha: 255 }, Color { red: 95, green: 175, blue: 135, alpha: 255 },
    Color { red: 95, green: 175, blue: 175, alpha: 255 }, Color { red: 95, green: 175, blue: 215, alpha: 255 }, Color { red: 95, green: 175, blue: 255, alpha: 255 },

    Color { red: 95, green: 215, blue: 0, alpha: 255 }, Color { red: 95, green: 215, blue: 95, alpha: 255 }, Color { red: 95, green: 215, blue: 135, alpha: 255 },
    Color { red: 95, green: 215, blue: 175, alpha: 255 }, Color { red: 95, green: 215, blue: 215, alpha: 255 }, Color { red: 95, green: 215, blue: 255, alpha: 255 },

    Color { red: 95, green: 255, blue: 0, alpha: 255 }, Color { red: 95, green: 255, blue: 95, alpha: 255 }, Color { red: 95, green: 255, blue: 135, alpha: 255 },
    Color { red: 95, green: 255, blue: 175, alpha: 255 }, Color { red: 95, green: 255, blue: 215, alpha: 255 }, Color { red: 95, green: 255, blue: 255, alpha: 255 },

    Color { red: 135, green: 0, blue: 0, alpha: 255 }, Color { red: 135, green: 0, blue: 95, alpha: 255 }, Color { red: 135, green: 0, blue: 135, alpha: 255 },
    Color { red: 135, green: 0, blue: 175, alpha: 255 }, Color { red: 135, green: 0, blue: 215, alpha: 255 }, Color { red: 135, green: 0, blue: 255, alpha: 255 },

    Color { red: 135, green: 95, blue: 0, alpha: 255 }, Color { red: 135, green: 95, blue: 95, alpha: 255 }, Color { red: 135, green: 95, blue: 135, alpha: 255 },
    Color { red: 135, green: 95, blue: 175, alpha: 255 }, Color { red: 135, green: 95, blue: 215, alpha: 255 }, Color { red: 135, green: 95, blue: 255, alpha: 255 },

    Color { red: 135, green: 135, blue: 0, alpha: 255 }, Color { red: 135, green: 135, blue: 95, alpha: 255 }, Color { red: 135, green: 135, blue: 135, alpha: 255 },
    Color { red: 135, green: 135, blue: 175, alpha: 255 }, Color { red: 135, green: 135, blue: 215, alpha: 255 }, Color { red: 135, green: 135, blue: 255, alpha: 255 },

    Color { red: 135, green: 175, blue: 0, alpha: 255 }, Color { red: 135, green: 175, blue: 95, alpha: 255 }, Color { red: 135, green: 175, blue: 135, alpha: 255 },
    Color { red: 135, green: 175, blue: 175, alpha: 255 }, Color { red: 135, green: 175, blue: 215, alpha: 255 }, Color { red: 135, green: 175, blue: 255, alpha: 255 },

    Color { red: 135, green: 215, blue: 0, alpha: 255 }, Color { red: 135, green: 215, blue: 95, alpha: 255 }, Color { red: 135, green: 215, blue: 135, alpha: 255 },
    Color { red: 135, green: 215, blue: 175, alpha: 255 }, Color { red: 135, green: 215, blue: 215, alpha: 255 }, Color { red: 135, green: 215, blue: 255, alpha: 255 },

    Color { red: 135, green: 255, blue: 0, alpha: 255 }, Color { red: 135, green: 255, blue: 95, alpha: 255 }, Color { red: 135, green: 255, blue: 135, alpha: 255 },
    Color { red: 135, green: 255, blue: 175, alpha: 255 }, Color { red: 135, green: 255, blue: 215, alpha: 255 }, Color { red: 135, green: 255, blue: 255, alpha: 255 },

    Color { red: 175, green: 0, blue: 0, alpha: 255 }, Color { red: 175, green: 0, blue: 95, alpha: 255 }, Color { red: 175, green: 0, blue: 135, alpha: 255 },
    Color { red: 175, green: 0, blue: 175, alpha: 255 }, Color { red: 175, green: 0, blue: 215, alpha: 255 }, Color { red: 175, green: 0, blue: 255, alpha: 255 },

    Color { red: 175, green: 95, blue: 0, alpha: 255 }, Color { red: 175, green: 95, blue: 95, alpha: 255 }, Color { red: 175, green: 95, blue: 135, alpha: 255 },
    Color { red: 175, green: 95, blue: 175, alpha: 255 }, Color { red: 175, green: 95, blue: 215, alpha: 255 }, Color { red: 175, green: 95, blue: 255, alpha: 255 },

    Color { red: 175, green: 135, blue: 0, alpha: 255 }, Color { red: 175, green: 135, blue: 95, alpha: 255 }, Color { red: 175, green: 135, blue: 135, alpha: 255 },
    Color { red: 175, green: 135, blue: 175, alpha: 255 }, Color { red: 175, green: 135, blue: 215, alpha: 255 }, Color { red: 175, green: 135, blue: 255, alpha: 255 },

    Color { red: 175, green: 175, blue: 0, alpha: 255 }, Color { red: 175, green: 175, blue: 95, alpha: 255 }, Color { red: 175, green: 175, blue: 135, alpha: 255 },
    Color { red: 175, green: 175, blue: 175, alpha: 255 }, Color { red: 175, green: 175, blue: 215, alpha: 255 }, Color { red: 175, green: 175, blue: 255, alpha: 255 },

    Color { red: 175, green: 215, blue: 0, alpha: 255 }, Color { red: 175, green: 215, blue: 95, alpha: 255 }, Color { red: 175, green: 215, blue: 135, alpha: 255 },
    Color { red: 175, green: 215, blue: 175, alpha: 255 }, Color { red: 175, green: 215, blue: 215, alpha: 255 }, Color { red: 175, green: 215, blue: 255, alpha: 255 },

    Color { red: 175, green: 255, blue: 0, alpha: 255 }, Color { red: 175, green: 255, blue: 95, alpha: 255 }, Color { red: 175, green: 255, blue: 135, alpha: 255 },
    Color { red: 175, green: 255, blue: 175, alpha: 255 }, Color { red: 175, green: 255, blue: 215, alpha: 255 }, Color { red: 175, green: 255, blue: 255, alpha: 255 },

    Color { red: 215, green: 0, blue: 0, alpha: 255 }, Color { red: 215, green: 0, blue: 95, alpha: 255 }, Color { red: 215, green: 0, blue: 135, alpha: 255 },
    Color { red: 215, green: 0, blue: 175, alpha: 255 }, Color { red: 215, green: 0, blue: 215, alpha: 255 }, Color { red: 215, green: 0, blue: 255, alpha: 255 },

    Color { red: 215, green: 95, blue: 0, alpha: 255 }, Color { red: 215, green: 95, blue: 95, alpha: 255 }, Color { red: 215, green: 95, blue: 135, alpha: 255 },
    Color { red: 215, green: 95, blue: 175, alpha: 255 }, Color { red: 215, green: 95, blue: 215, alpha: 255 }, Color { red: 215, green: 95, blue: 255, alpha: 255 },

    Color { red: 215, green: 135, blue: 0, alpha: 255 }, Color { red: 215, green: 135, blue: 95, alpha: 255 }, Color { red: 215, green: 135, blue: 135, alpha: 255 },
    Color { red: 215, green: 135, blue: 175, alpha: 255 }, Color { red: 215, green: 135, blue: 215, alpha: 255 }, Color { red: 215, green: 135, blue: 255, alpha: 255 },

    Color { red: 215, green: 175, blue: 0, alpha: 255 }, Color { red: 215, green: 175, blue: 95, alpha: 255 }, Color { red: 215, green: 175, blue: 135, alpha: 255 },
    Color { red: 215, green: 175, blue: 175, alpha: 255 }, Color { red: 215, green: 175, blue: 215, alpha: 255 }, Color { red: 215, green: 175, blue: 255, alpha: 255 },

    Color { red: 215, green: 215, blue: 0, alpha: 255 }, Color { red: 215, green: 215, blue: 95, alpha: 255 }, Color { red: 215, green: 215, blue: 135, alpha: 255 },
    Color { red: 215, green: 215, blue: 175, alpha: 255 }, Color { red: 215, green: 215, blue: 215, alpha: 255 }, Color { red: 215, green: 215, blue: 255, alpha: 255 },

    Color { red: 215, green: 255, blue: 0, alpha: 255 }, Color { red: 215, green: 255, blue: 95, alpha: 255 }, Color { red: 215, green: 255, blue: 135, alpha: 255 },
    Color { red: 215, green: 255, blue: 175, alpha: 255 }, Color { red: 215, green: 255, blue: 215, alpha: 255 }, Color { red: 215, green: 255, blue: 255, alpha: 255 },

    Color { red: 255, green: 0, blue: 0, alpha: 255 }, Color { red: 255, green: 0, blue: 95, alpha: 255 }, Color { red: 255, green: 0, blue: 135, alpha: 255 },
    Color { red: 255, green: 0, blue: 175, alpha: 255 }, Color { red: 255, green: 0, blue: 215, alpha: 255 }, Color { red: 255, green: 0, blue: 255, alpha: 255 },

    Color { red: 255, green: 95, blue: 0, alpha: 255 }, Color { red: 255, green: 95, blue: 95, alpha: 255 }, Color { red: 255, green: 95, blue: 135, alpha: 255 },
    Color { red: 255, green: 95, blue: 175, alpha: 255 }, Color { red: 255, green: 95, blue: 215, alpha: 255 }, Color { red: 255, green: 95, blue: 255, alpha: 255 },

    Color { red: 255, green: 135, blue: 0, alpha: 255 }, Color { red: 255, green: 135, blue: 95, alpha: 255 }, Color { red: 255, green: 135, blue: 135, alpha: 255 },
    Color { red: 255, green: 135, blue: 175, alpha: 255 }, Color { red: 255, green: 135, blue: 215, alpha: 255 }, Color { red: 255, green: 135, blue: 255, alpha: 255 },

    Color { red: 255, green: 175, blue: 0, alpha: 255 }, Color { red: 255, green: 175, blue: 95, alpha: 255 }, Color { red: 255, green: 175, blue: 135, alpha: 255 },
    Color { red: 255, green: 175, blue: 175, alpha: 255 }, Color { red: 255, green: 175, blue: 215, alpha: 255 }, Color { red: 255, green: 175, blue: 255, alpha: 255 },

    Color { red: 255, green: 215, blue: 0, alpha: 255 }, Color { red: 255, green: 215, blue: 95, alpha: 255 }, Color { red: 255, green: 215, blue: 135, alpha: 255 },
    Color { red: 255, green: 215, blue: 175, alpha: 255 }, Color { red: 255, green: 215, blue: 215, alpha: 255 }, Color { red: 255, green: 215, blue: 255, alpha: 255 },

    Color { red: 255, green: 255, blue: 0, alpha: 255 }, Color { red: 255, green: 255, blue: 95, alpha: 255 }, Color { red: 255, green: 255, blue: 135, alpha: 255 },
    Color { red: 255, green: 255, blue: 175, alpha: 255 }, Color { red: 255, green: 255, blue: 215, alpha: 255 }, Color { red: 255, green: 255, blue: 255, alpha: 255 },

    // 24 grayscale Colors
    Color { red: 8, green: 8, blue: 8, alpha: 255 }, Color { red: 18, green: 18, blue: 18, alpha: 255 }, Color { red: 28, green: 28, blue: 28, alpha: 255 },
    Color { red: 38, green: 38, blue: 38, alpha: 255 }, Color { red: 48, green: 48, blue: 48, alpha: 255 }, Color { red: 58, green: 58, blue: 58, alpha: 255 },
    Color { red: 68, green: 68, blue: 68, alpha: 255 }, Color { red: 78, green: 78, blue: 78, alpha: 255 }, Color { red: 88, green: 88, blue: 88, alpha: 255 },
    Color { red: 98, green: 98, blue: 98, alpha: 255 }, Color { red: 108, green: 108, blue: 108, alpha: 255 }, Color { red: 118, green: 118, blue: 118, alpha: 255 },
    Color { red: 128, green: 128, blue: 128, alpha: 255 }, Color { red: 138, green: 138, blue: 138, alpha: 255 }, Color { red: 148, green: 148, blue: 148, alpha: 255 },
    Color { red: 158, green: 158, blue: 158, alpha: 255 }, Color { red: 168, green: 168, blue: 168, alpha: 255 }, Color { red: 178, green: 178, blue: 178, alpha: 255 },
    Color { red: 188, green: 188, blue: 188, alpha: 255 }, Color { red: 198, green: 198, blue: 198, alpha: 255 }, Color { red: 208, green: 208, blue: 208, alpha: 255 },
    Color { red: 218, green: 218, blue: 218, alpha: 255 }, Color { red: 228, green: 228, blue: 228, alpha: 255 }, Color { red: 238, green: 238, blue: 238, alpha: 255 }
];

#[allow(dead_code)]
#[repr(u8)]
pub enum Color8 {
    Black = 0,
    Red = 1,
    Green = 2,
    Yellow = 3,
    Blue = 4,
    Magenta = 5,
    Cyan = 6,
    White = 7,
}

#[allow(dead_code)]
#[repr(u8)]
pub enum GraphicRendition {
    Normal = 0,
    Bright = 1,
    Dim = 2,
    Italic = 3,
    Underline = 4,
    SlowBlink = 5,
    FastBlink = 6,
    Invert = 7,
    ResetBrightDim = 22,
    ResetItalic = 23,
    ResetUnderline = 24,
    ResetBlink = 25,
    ResetInvert = 27
}

#[allow(dead_code)]
#[repr(i16)]
pub enum Key {
    KeyUp = 0x0100,
    KeyDown = 0x0101,
    KeyRight = 0x0102,
    KeyLeft = 0x0103
}

pub fn color_test() {
    println!("4-bit colors:");
    for i in 0 .. 16 {
        print!("{} ", bg_8bit_color(i));
    }
    println!("{}\n", RESET);

    print!("8-bit colors:");
    for i in 0 .. 216 {
        if i % 36 == 0 {
            println!("{}", RESET);
        }

        print!("{} ", bg_8bit_color(i + 16));
    }
    println!("{}\n", RESET);

    println!("Grayscale colors:");
    for i in 0 .. 24 {
        print!("{} ", bg_8bit_color(i + 232));
    }
    println!("{}\n", RESET);
}

pub fn fg_8bit_color(color_index: u8) -> String {
    return format!("\x1b[38;5;{}m", color_index);
}

pub fn bg_8bit_color(color_index: u8) -> String {
    return format!("\x1b[48;5;{}m", color_index);
}

pub fn fg_24bit_color(color: Color) -> String {
    return format!("\x1b[38;2;{};{};{}m", color.red, color.green, color.blue);
}

pub fn bg_24bit_color(color: Color) -> String {
    return format!("\x1b[48;2;{};{};{}m", color.red, color.green, color.blue);
}