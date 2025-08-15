#![no_std]

extern crate alloc;

use alloc::format;
#[allow(unused_imports)]
use runtime::*;
#[allow(unused_imports)]
use libc::*;
use alloc::vec::Vec;
use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use chrono::{Datelike, Timelike};
use spin::{Once, RwLock};
use concurrent::thread;
use ::time::{date, systime};
use graphic::{color, map_framebuffer, FramebufferInfo};
use graphic::lfb::{DEFAULT_CHAR_HEIGHT, LFB};
use libc::time::time::tm;
use naming::shared_types::{OpenOptions, SeekOrigin};
use terminal::{print, println};

unsafe extern "C" {
    /// Get the size of the `gb_s` structure (implemented in `peanut-gb.c`).
    /// This struct holds the entire state of the emulated Game Boy.
    /// Since we do not have a Rust binding for this, we use a C function to get the size.
    fn gb_size() -> c_int;

    /// Get a pointer to the joypad state in the `gb_s` structure (implemented in `peanut-gb.c`).
    /// The joypad state is a single byte where each bit represents a button state.
    /// If no button is pressed, all bits are set to 1 (0xff).
    /// The buttons are represented by the `JoypadButton` enum.
    fn gb_get_joypad_ptr(gb: *mut c_void) -> *mut u8;

    /// Initialization function for the PeanutGB emulator.
    /// The `gb` parameter must point to block of memory large enough to hold the `gb_s` structure.
    /// The size of this structure can be obtained by calling `gb_size()`.
    /// The `priv_data` parameter can be used to pass additional data to the emulator,
    /// but is currently unused in this implementation.
    /// The other parameters are function pointers and crucial for the emulator to function.
    fn gb_init(gb: *mut c_void,
               gb_rom_read: unsafe extern "C" fn(*mut c_void, u32) -> u8,
               gb_cart_ram_read: unsafe extern "C" fn(*mut c_void, u32) -> u8,
               gb_cart_ram_write: unsafe extern "C" fn(*mut c_void, u32, u8),
               gb_error: unsafe extern "C" fn(*mut c_void, i32, u16),
               priv_data: *const c_void) -> c_int;

    /// Initialize the LCD of the PeanutGB emulator.
    /// This function must be called after the emulator has been initialized.
    /// If this function is not called, the emulator will work, but not render any graphics.
    fn gb_init_lcd(gb: *mut c_void, lcd_draw_line: *const c_void);

    /// Run a single frame of the PeanutGB emulator.
    /// This function must be called in a loop to run the emulator.
    /// To maintain a stable frame rate, the caller should measure the time taken by this function
    /// and sleep for the remaining time to achieve the desired frame rate.
    /// Otherwise, the emulator will run as fast as possible.
    fn gb_run_frame(gb: *mut c_void);

    /// Many Game Boy games have a battery backed RAM that is used to save the game state.
    /// This function returns the size of the save RAM in bytes, which is necessary
    /// to allocate memory for the `gb_cart_ram_read` and `gb_cart_ram_write` functions.
    fn gb_get_save_size(gb: *mut c_void) -> u32;

    /// Get the name of the ROM currently loaded in the PeanutGB emulator.
    /// The name is returned as a C string (null-terminated).
    fn gb_get_rom_name(gb: *mut c_void, title_str: *const c_char) -> *const c_char;

    /// Set the real-time clock (RTC) of the PeanutGB emulator.
    /// This function enables the RTC functionality in the emulator for games that use it.
    /// It needs to be called once before running the emulator.
    /// Afterward, the emulator updates the RTC automatically.
    fn gb_set_rtc(gb: *mut c_void, rtc: *const tm) -> c_int;
}

/// Bitmask for the joypad buttons. See `gb_get_joypad_ptr` for more details.
#[repr(u8)]
enum JoypadButton {
    A = 0x01,
    B = 0x02,
    Select = 0x04,
    Start = 0x08,
    Right = 0x10,
    Left = 0x20,
    Up = 0x40,
    Down = 0x80,
}

/// Error codes used in `gb_error`.
#[derive(Debug)]
enum GbError {
    GbUnknownError = 0,
    GbInvalidOpcode = 1,
    GbInvalidRead = 2,
    GbInvalidWrite = 3,
}

impl TryFrom<c_int> for GbError {
    type Error = ();

    fn try_from(value: c_int) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(GbError::GbUnknownError),
            1 => Ok(GbError::GbInvalidOpcode),
            2 => Ok(GbError::GbInvalidRead),
            3 => Ok(GbError::GbInvalidWrite),
            _ => Err(())
        }
    }
}

/// The target frame rate for the emulator.
/// The original Game Boy runs at 60 frames per second.
/// Increasing this value will make the emulator run faster,
/// decreasing it will make the emulator run slower.
const TARGET_FRAME_RATE: usize = 60;

/// The number of milliseconds per frame at the target frame rate.
const MS_PER_FRAME: usize = 1000 / TARGET_FRAME_RATE;

/// The original Game Boy screen resolution (160x144 pixels).
const GB_SCREEN_RES: (u32, u32) = (160, 144);

/// The Game Boy screen is rendered using this scale factor.
const SCALE: u32 = 2;

/// The color palette used for rendering.
/// The Game Boy supports 4 shades of gray, represented as 32-bit ARGB colors in this array.
static PALETTE: &[u32] = &[
    0xe0f8d0, // White
    0x88c070, // Light Gray
    0x346856, // Dark Gray
    0x081820, // Black
];

/// The ROM file to be played by emulator.
static ROM: RwLock<Vec<u8>> = RwLock::new(Vec::new());

/// The save RAM for the current game.
/// It is initialized to the size returned by `gb_get_save_size`.
static RAM: RwLock<Vec<u8>> = RwLock::new(Vec::new());

/// The framebuffer information for rendering.
static FRAMEBUFFER: Once<FramebufferInfo> = Once::new();

/// The screen offset for centering the rendered Game Boy screen in the framebuffer.
static SCREEN_OFFSET: Once<(u32, u32)> = Once::new();

/// Read a byte from the ROM file at the offset specified by `addr`.
/// This is a callback function for the PeanutGB emulator.
pub unsafe extern "C" fn gb_rom_read(_gb: *mut c_void, addr: u32) -> u8 {
    let rom = ROM.read();
    rom[addr as usize]
}

/// Read a byte from the save RAM at the offset specified by `addr`.
/// This is a callback function for the PeanutGB emulator.
pub unsafe extern "C" fn gb_cart_ram_read(_gb: *mut c_void, addr: u32) -> u8 {
    let ram = RAM.read();
    ram[addr as usize]
}

/// Write a byte to the save RAM at the offset specified by `addr`.
/// This is a callback function for the PeanutGB emulator.
pub unsafe extern "C" fn gb_cart_ram_write(_gb: *mut c_void, addr: u32, val: u8) {
    let mut ram = RAM.write();
    ram[addr as usize] = val;
}

/// Handle emulation errors.
/// This is a callback function for the PeanutGB emulator.
pub unsafe extern "C" fn gb_error(_gb: *mut c_void, error: c_int, addr: u16) {
    let error = GbError::try_from(error).unwrap_or(GbError::GbUnknownError);
    panic!("PeanutGB error [{:?}] at address [0x{:0>4x}]!", error, addr);
}

/// Draw a line of pixels from the Game Boy screen to the framebuffer.
/// The buffer pointed to by `pixels` contains the pixel data for the line.
/// Each pixel is represented by a single byte, whose first two bits represent the color index.
/// The other bits are used for Game Boy Color emulation, but are ignored in this implementation.
pub unsafe extern "C" fn lcd_draw_line(_gb: *mut c_void, pixels: *const u8, line: u8) {
    let fb_info  = FRAMEBUFFER.get().unwrap();
    let (x_offset, y_offset) = SCREEN_OFFSET.get().unwrap();
    let res_x = GB_SCREEN_RES.0 * SCALE;

    let mut fb_ptr = (fb_info.addr + (x_offset * 4 + (y_offset + line as u32 * SCALE) * fb_info.pitch) as u64) as *mut u32;

    unsafe {
        for _y in 0..SCALE {
            for x in 0..res_x {
                let color_index = pixels.offset((x / SCALE) as isize).read() as usize & 0x03; // Get the color index (0-3)
                let color = PALETTE[color_index];

                fb_ptr.add(x as usize).write(color);
            }

            fb_ptr = fb_ptr.add((fb_info.pitch / 4) as usize); // Move to the next line in the framebuffer
        }
    }
}

/// Read the ROM file from the specified path and load it into the `ROM` buffer.
fn read_rom(path: &str) {
    let file = naming::open(&path, OpenOptions::READONLY).expect("Failed to open ROM file");
    let file_size = naming::seek(file, 0, SeekOrigin::End).expect("Failed to get ROM file size");
    naming::seek(file, 0, SeekOrigin::Start).expect("Failed to get ROM file offset");

    let mut rom = ROM.write();
    for _ in 0..file_size {
        rom.push(0)
    }

    naming::read(file, rom.as_mut_slice()).expect("Failed to read ROM file");
}

#[unsafe(no_mangle)]
pub fn main() {
    // Read the rom file into the `ROM` buffer.
    let mut args = env::args();
    let path = args.nth(1).expect("Usage: peanut-gb <rom_path>");
    read_rom(path.as_str());

    // Allocate memory for the `gb_s` structure
    let gb_size = unsafe { gb_size() } as usize;
    let mut gb = Vec::<u8>::with_capacity(gb_size);

    // Get a mutable pointer to the allocated memory for convenience
    let gb_ptr = gb.as_mut_ptr() as *mut c_void;

    // Reference to the joypad state
    let gb_joypad: &mut u8;

    // Initialize the PeanutGB emulator and get a pointer to the joypad state
    unsafe {
        let init_result = gb_init(gb_ptr, gb_rom_read, gb_cart_ram_read, gb_cart_ram_write, gb_error, ptr::null());
        if init_result != 0 {
            panic!("Failed to initialize PeanutGB!");
        }

        gb_init_lcd(gb_ptr, lcd_draw_line as *const c_void);

        gb_joypad = &mut *gb_get_joypad_ptr(gb_ptr);
    }

    // Initialize the save RAM
    unsafe {
        let ram_size = gb_get_save_size(gb_ptr);

        let mut name_buffer = [0 as c_char; 16];
        gb_get_rom_name(gb_ptr, name_buffer.as_mut_ptr() as *mut c_char);
        let rom_name = core::ffi::CStr::from_ptr(name_buffer.as_ptr()).to_str().unwrap();

        let mut ram = RAM.write();
        for _ in 0..ram_size {
            ram.push(0);
        }

        println!("Loaded ROM: {}", rom_name);
        println!("ROM size: {}", ROM.read().len());
        println!("RAM size: {}", ram_size);
    }

    // Initialize the real-time clock (RTC) if needed.
    unsafe {
        let date = date();
        let tm = tm {
            tm_sec: date.time().second() as c_int,
            tm_min: date.time().minute() as c_int,
            tm_hour: date.time().hour() as c_int,
            tm_mday: date.date_naive().day() as c_int,
            tm_mon: date.date_naive().month0() as c_int,
            tm_year: date.year() as c_int,
            tm_wday: date.weekday() as c_int,
            tm_yday: date.ordinal0() as c_int,
            tm_isdst: -1
        };

        gb_set_rtc(gb_ptr, ptr::from_ref(&tm));
    }

    // Initialize the framebuffer
    FRAMEBUFFER.call_once(|| map_framebuffer().unwrap());

    let fb_info  = FRAMEBUFFER.get().unwrap();
    let lfb = LFB::new(fb_info.addr as *mut u8, fb_info.pitch, fb_info.width, fb_info.height, fb_info.bpp);
    let x_offset = (fb_info.width - GB_SCREEN_RES.0 * SCALE) / 2;
    let y_offset = (fb_info.height - GB_SCREEN_RES.1 * SCALE) / 2;
    SCREEN_OFFSET.call_once(|| (x_offset, y_offset));

    println!("\nUp/Down/Left/Right = WASD\nA = J, B = K\nStart = Space\nSelect = Enter\nQuit = Q");

    let mut fps = 0;
    let mut fps_timer = 0;

    // Run the emulator loop until 'q' is pressed
    loop {
        let time = systime();

        // Check if a key has been pressed and update the joypad state accordingly
        if let Some(key) = terminal::read::read_nb() {
            match key {
                ' ' => *gb_joypad = !(JoypadButton::Start as u8),
                '\n' => *gb_joypad = !(JoypadButton::Select as u8),
                'w' => *gb_joypad = !(JoypadButton::Up as u8),
                's' => *gb_joypad = !(JoypadButton::Down as u8),
                'a' => *gb_joypad = !(JoypadButton::Left as u8),
                'd' => *gb_joypad = !(JoypadButton::Right as u8),
                'j' => *gb_joypad = !(JoypadButton::A as u8),
                'k' => *gb_joypad = !(JoypadButton::B as u8),
                'q' => {
                    println!("\nExiting PeanutGB emulator...");
                    break; // Exit the emulator loop
                }
                _ => {}
            }
        }

        // Emulate a single frame
        unsafe { gb_run_frame(gb_ptr) };

        // Reset the joypad state to 0xff (no buttons pressed)
        // Currently, there is no way to check if a key is pressed or released.
        // We can just read characters from the terminal.
        // For each character read, we emulate a button press.
        // This will make most games unplayable and will be updated
        // once D3OS offers a proper input API.
        *gb_joypad = 0xff;

        // Calculate the elapsed time since the start of the frame
        let elapsed = systime() - time;

        // Sleep to maintain the target frame rate
        if elapsed.num_milliseconds() < MS_PER_FRAME as i64 {
            // Sleep for the remaining time to maintain 60 FPS
            let sleep_time = MS_PER_FRAME as i64 - elapsed.num_milliseconds();
            thread::sleep(sleep_time as usize);
        }

        let elapsed = systime() - time;
        fps_timer += elapsed.num_milliseconds() as usize;
        fps += 1;

        if fps_timer >= 1000 {
            let offset = SCREEN_OFFSET.get().unwrap();
            lfb.draw_string(offset.0, offset.1 - DEFAULT_CHAR_HEIGHT, color::WHITE, color::BLACK, &format!("{} fps", fps));

            fps_timer = 0;
            fps = 0;
        }
    }
}