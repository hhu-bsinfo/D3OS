use core::ptr;

use alloc::sync::Arc;
use concurrent::thread;
use graphic::lfb;

use crate::{lfb_terminal::LFBTerminal, terminal::Terminal};

const CURSOR: char = if let Some(cursor) = char::from_u32(0x2588) {
    cursor
} else {
    '_'
};

const CURSOR_UPDATE_INTERVAL: usize = 250;

pub struct CursorState {
    pub(crate) pos: (u16, u16),
    pub(crate) saved_pos: (u16, u16),
}

pub struct CursorThread {
    terminal: Arc<dyn Terminal>,
    visible: bool,
}

impl CursorState {
    pub const fn new() -> Self {
        Self {
            pos: (0, 1),
            saved_pos: (0, 1),
        }
    }
}

impl CursorThread {
    pub fn new(terminal: Arc<dyn Terminal>) -> Self {
        Self {
            terminal,
            visible: true,
        }
    }

    pub fn run(&mut self) {
        let mut sleep_counter = 0usize;

        loop {
            thread::sleep(CURSOR_UPDATE_INTERVAL);
            sleep_counter += CURSOR_UPDATE_INTERVAL;

            // CAUTION: This only works because LFBTerminal is the only implementation of Terminal
            let terminal = unsafe {
                (ptr::from_ref(self.terminal.as_ref()) as *const LFBTerminal)
                    .as_ref()
                    .unwrap()
            };

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

            if sleep_counter >= 1000 {
                LFBTerminal::draw_status_bar(&mut display);
                sleep_counter = 0;
            }
        }
    }
}
