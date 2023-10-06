use x86_64::instructions::port::{Port, PortWriteOnly};
use crate::devices::cga::Color::{Black, LightGray};

// make type comparable, printable and enable copy semantics
#[allow(dead_code)]   // avoid warnings for unused colors
#[repr(u8)]           // store each enum variant as an u8
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Pink = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    LightPink = 13,
    Yellow = 14,
    White = 15,
}

const CGA_MEMORY: *mut u8 = 0xb8000 as *mut u8;
const CGA_ROWS: u16 = 25;
const CGA_COLUMNS: u16 = 80;

const CURSOR_HIGH_BYTE: u8 = 0x0e;
const CURSOR_LOW_BYTE: u8 = 0x0f;

static mut INDEX_PORT: PortWriteOnly<u8> = PortWriteOnly::new(0x3d4);
static mut DATA_PORT: Port<u8> = Port::new(0x3d5);

pub fn clear() {
    let cga_memory = CGA_MEMORY as *mut u16;
    for i in 0..(CGA_ROWS * CGA_COLUMNS) {
        unsafe { cga_memory.offset(i as isize).write(0x0700); }
    }
}

pub fn show(x: u16, y: u16, character: char, attrib: u8) {
    if x > CGA_COLUMNS || y > CGA_ROWS {
		return ; 
    }
    
    let pos = ((y * CGA_COLUMNS + x) * 2) as isize;

    unsafe {
        *CGA_MEMORY.offset(pos) = character as u8;
        *CGA_MEMORY.offset(pos + 1) = attrib;
    }
}

pub fn getpos() -> (u16, u16) {
    let low: u8;
    let high: u8;

    unsafe {
        INDEX_PORT.write(CURSOR_HIGH_BYTE);
        high = DATA_PORT.read();
        INDEX_PORT.write(CURSOR_LOW_BYTE);
        low = DATA_PORT.read();
    }

    let pos = (low as u16) | ((high as u16) << 8);
    return (pos % CGA_COLUMNS, pos / CGA_COLUMNS);
}

pub fn setpos(x: u16, y: u16) {
    let pos: u16 = y * CGA_COLUMNS + x;
    let low: u8 = (pos & 0xff) as u8;
    let high: u8 = ((pos >> 8) & 0xff) as u8;

    unsafe {
        INDEX_PORT.write(CURSOR_HIGH_BYTE);
        DATA_PORT.write(high);
        INDEX_PORT.write(CURSOR_LOW_BYTE);
        DATA_PORT.write(low);
    }
}

pub fn print_char(c: char) {
    let mut pos = getpos();

    if c == '\n' {
        pos.1 += 1;
        pos.0 = 0;
    } else {
        show(pos.0, pos.1, c, attribute(Black, LightGray));
        pos.0 += 1;
    }

    if pos.0 >= CGA_COLUMNS {
        pos.1 += 1;
        pos.0 = 0;
    }

    if pos.1 >= CGA_ROWS {
        scroll_up();
        pos.0 = 0;
        pos.1 = CGA_ROWS - 1;
    }

    setpos(pos.0, pos.1);
}

pub fn print_str(string: &str) {
    for c in string.chars() {
        print_char(c);
    }
}

pub fn scroll_up() {
    unsafe {
        CGA_MEMORY.copy_from(CGA_MEMORY.offset((CGA_COLUMNS * 2) as isize), (CGA_COLUMNS * (CGA_ROWS - 1) * 2) as usize);

        let last_line = CGA_MEMORY.offset((CGA_COLUMNS * (CGA_ROWS - 1) * 2) as isize) as *mut u16;
        for i in 0..CGA_COLUMNS {
            last_line.offset(i as isize).write(0x0700);
        }
    }
}

pub fn attribute(bg: Color, fg: Color) -> u8 {
    (bg as u8) << 4 | (fg as u8)
}
