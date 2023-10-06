/*****************************************************************************
 *                                                                           *
 *                                k e y                                      *
 *                                                                           *
 *---------------------------------------------------------------------------*
 * Beschreibung:    Taste, bestehend aus ASCII-, Scan-Code und Modifier-Bits.*
 *                                                                           *
 * Autor:           Michael Schoettner, HHU Duesseldorf, 17.1.2022           *
 *****************************************************************************/

#![allow(unused_variables)]
#![allow(dead_code)]

/* modifier bits */
const KMOD_SHIFT: u8 = 1;
const KMOD_ALT_LEFT: u8 = 2;
const KMOD_ALT_RIGHT: u8 = 4;
const KMOD_CTRL_LEFT: u8 = 8;
const KMOD_CTRL_RIGHT: u8 = 16;
const KMOD_CAPS_LOCK: u8 = 32;
const KMOD_NUM_LOCK: u8 = 64;
const KMOD_SCROLL_LOCK: u8 = 128;


pub const SCAN_F1: u8 = 0x3b;
pub const SCAN_DEL: u8 = 0x53;
pub const SCAN_UP: u8 = 72;
pub const SCAN_DOWN: u8 = 80;
pub const SCAN_LEFT: u8 = 75;
pub const SCAN_RIGHT: u8 = 77;
pub const SCAN_DIV: u8 = 8;

// fields must be public because of static KB in keyboard.rs
#[derive(Copy, Clone, Default)]
pub struct Key {
    pub asc: u8,
    // ASCII code
    pub scan: u8,
    // scan code
    pub modi: u8,     // modifier
}

impl Key {
    pub fn new(a: u8, s: u8, m: u8) -> Key {
        Key { asc: a, scan: s, modi: m }
    }

    // VALID: mit Scancode = 0 werden ungueltige Tasten gekennzeichnet.
    pub fn valid(&mut self) -> bool { return self.scan != 0; }

    // INVALIDATE: setzt den Scancode auf Null und sorgt somit fuer einen
    //             ungueltigen Tastencode.
    pub fn set_invalidate(&mut self) { self.scan = 0; }

    // ASCII, SCANCODE: Setzen und Abfragen von Ascii und Scancode
    pub fn set_ascii(&mut self, a: u8) { self.asc = a; }
    pub fn set_scancode(&mut self, s: u8) { self.scan = s; }
    pub fn get_ascii(&mut self) -> u8 { return self.asc; }
    pub fn get_scancode(&mut self) -> u8 { return self.scan; }

    //
    // Funktionen zum Setzen und Loeschen von SHIFT, ALT, CTRL usw.
    //
    pub fn set_shift(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_SHIFT; } else { self.modi = self.modi & !KMOD_SHIFT; }
    }

    pub fn set_alt_left(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_ALT_LEFT; } else { self.modi = self.modi & !KMOD_ALT_LEFT; }
    }

    pub fn set_alt_right(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_ALT_RIGHT; } else { self.modi = self.modi & !KMOD_ALT_RIGHT; }
    }

    pub fn set_ctrl_left(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_CTRL_LEFT; } else { self.modi = self.modi & !KMOD_CTRL_LEFT; }
    }

    pub fn set_ctrl_right(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_CTRL_RIGHT; } else { self.modi = self.modi & !KMOD_CTRL_RIGHT; }
    }

    pub fn set_caps_lock(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_CAPS_LOCK; } else { self.modi = self.modi & !KMOD_CAPS_LOCK; }
    }

    pub fn set_num_lock(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_NUM_LOCK; } else { self.modi = self.modi & !KMOD_NUM_LOCK; }
    }

    pub fn set_scroll_lock(&mut self, pressed: bool) {
        if pressed == true { self.modi = self.modi | KMOD_SCROLL_LOCK; } else { self.modi = self.modi & !KMOD_SCROLL_LOCK; }
    }


    //
    // Funktionen zum Abfragen von SHIFT, ALT, CTRL usw.
    //
    pub fn get_shift(&self) -> bool { return (self.modi & KMOD_SHIFT) != 0; }
    pub fn get_alt_left(&self) -> bool { return (self.modi & KMOD_ALT_LEFT) != 0; }
    pub fn get_alt_right(&self) -> bool { return (self.modi & KMOD_ALT_RIGHT) != 0; }
    pub fn get_ctrl_left(&self) -> bool { return (self.modi & KMOD_CTRL_LEFT) != 0; }
    pub fn get_ctrl_right(&self) -> bool { return (self.modi & KMOD_CTRL_RIGHT) != 0; }
    pub fn get_caps_lock(&self) -> bool { return (self.modi & KMOD_CAPS_LOCK) != 0; }
    pub fn get_num_lock(&self) -> bool { return (self.modi & KMOD_NUM_LOCK) != 0; }
    pub fn get_scroll_lock(&self) -> bool { return (self.modi & KMOD_SCROLL_LOCK) != 0; }
    pub fn get_alt(&self) -> bool {
        return self.get_alt_left() || self.get_alt_right();
    }
    pub fn get_ctrl(&self) -> bool {
        return self.get_ctrl_left() || self.get_ctrl_right();
    }
}

