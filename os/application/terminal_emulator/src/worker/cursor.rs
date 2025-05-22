use core::cell::RefCell;

use alloc::rc::Rc;
use graphic::lfb;
use time::systime;

use crate::terminal::lfb_terminal::LFBTerminal;

use super::worker::Worker;

const UPDATE_INTERVAL: i64 = 250;

const CURSOR: char = match char::from_u32(0x2588) {
    Some(cursor) => cursor,
    None => '_',
};

pub struct Cursor {
    terminal: Rc<RefCell<LFBTerminal>>,
    visible: bool,
    last_tick: i64,
}

pub struct CursorState {
    pub(crate) pos: (u16, u16),
    pub(crate) saved_pos: (u16, u16),
}

impl Cursor {
    pub fn new(terminal: Rc<RefCell<LFBTerminal>>) -> Self {
        Self {
            terminal,
            visible: true,
            last_tick: 0,
        }
    }
}

impl Worker for Cursor {
    fn run(&mut self) {
        let terminal = self.terminal.borrow();
        let systime = systime().num_milliseconds();

        if systime < self.last_tick + UPDATE_INTERVAL {
            return;
        }
        self.last_tick = systime;

        let mut display = terminal.display.lock();
        let cursor = terminal.cursor.lock();
        let character =
            display.char_buffer[(cursor.pos.1 * display.size.0 + cursor.pos.0) as usize];

        let draw_character = match self.visible {
            true => match character.value {
                '\0' => ' ',
                value => value,
            },
            false => CURSOR,
        };

        display.lfb.direct_lfb().draw_char(
            cursor.pos.0 as u32 * lfb::DEFAULT_CHAR_WIDTH,
            cursor.pos.1 as u32 * lfb::DEFAULT_CHAR_HEIGHT,
            character.fg_color,
            character.bg_color,
            draw_character,
        );
        self.visible = !self.visible;
    }
}

impl CursorState {
    pub const fn new() -> Self {
        Self {
            pos: (0, 1),
            saved_pos: (0, 1),
        }
    }
}
