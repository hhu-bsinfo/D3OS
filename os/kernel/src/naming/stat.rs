/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: stat                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Meta data for each named object.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 30.12.2024, HHU                             ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

pub const MODE_FILE: u32 = 0x1;
pub const MODE_DIR: u32  = 0x2;
pub const MODE_LINK: u32 = 0x3;


#[derive(Debug, Copy, Clone)]
pub struct Stat {
    pub mode: Mode,
    pub size: usize,
    pub created_time: u64,
    pub modified_time: u64,
    pub accessed_time: u64,
}

impl Stat {
    pub fn new(mode: Mode, size: usize) -> Stat {
        Stat {
            mode,
            size,
            created_time: 0,
            modified_time: 0,
            accessed_time: 0,
        }
    }
    pub fn zeroed() -> Stat {
        Stat {
            mode: Mode::new(MODE_FILE),
            size: 0,
            created_time: 0,
            modified_time: 0,
            accessed_time: 0, 
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

    pub fn is_file(self) -> bool {
        (self.0 & MODE_FILE) == MODE_FILE
    }

    pub fn is_link(self) -> bool {
        (self.0 & MODE_LINK) == MODE_LINK
    }
}
