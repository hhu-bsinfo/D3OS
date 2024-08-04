/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: stat                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Meta data for each name service entry.                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 23.7.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::string::{String, ToString};


pub const MODE_CONT: u32 = 0b001u32;
pub const MODE_DIR: u32  = 0b010u32;
pub const MODE_LINK: u32 = 0b011u32;
pub const MODE_DEV: u32  = 0b100u32;


#[derive(Debug, Clone)]
pub struct Stat {
    pub name: String,
    pub mode: Mode,
    pub size: usize,
    pub ctime: u64, // creation time
    pub dev_id: u64, // for device files
}

impl Stat {
    pub fn new(name: String, mode: Mode, size: usize) -> Stat {
        Stat {
            name,
            mode,
            dev_id: 0,
            size,
            ctime: 0,
        }
    }
    pub fn zeroed() -> Stat {
        Stat {
            name: "".to_string(),
            mode: Mode::new(MODE_CONT),
            dev_id: 0,
            size: 0,
            ctime: 0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct Mode(u32);

impl Mode {
    pub fn new(value: u32) -> Mode {
        Mode(value)
    }

    pub fn is_directory(self) -> bool {
        (self.0 & MODE_DIR) == MODE_DIR
    }

    pub fn is_container(self) -> bool {
        (self.0 & MODE_CONT) == MODE_CONT
    }

    pub fn is_link(self) -> bool {
        (self.0 & MODE_LINK) == MODE_LINK
    }

    pub fn is_dev_entry(self) -> bool {
     (self.0 & MODE_DEV) == MODE_DEV
 }

}