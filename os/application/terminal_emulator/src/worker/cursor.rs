use core::ptr;

use alloc::sync::Arc;
use concurrent::thread::{self, Thread};
use graphic::lfb;

use crate::{
    terminal::{lfb_terminal::LFBTerminal, terminal::Terminal},
    terminal_emulator,
};

use super::worker::Worker;

const CURSOR_UPDATE_INTERVAL: usize = 250;

const CURSOR: char = match char::from_u32(0x2588) {
    Some(cursor) => cursor,
    None => '_',
};

pub struct Cursor {
    thread: Option<Thread>,
}

struct CursorThread {
    terminal: Arc<dyn Terminal>,
    visible: bool,
}

pub struct CursorState {
    pub(crate) pos: (u16, u16),
    pub(crate) saved_pos: (u16, u16),
}

impl Cursor {
    pub const fn new() -> Self {
        Self { thread: None }
    }
}

impl Worker for Cursor {
    fn create(&mut self) {
        if self.thread.is_some() {
            return;
        }

        let thread = thread::create(|| {
            let mut cursor = CursorThread::new(terminal_emulator().terminal());
            cursor.run();
        })
        .expect("Unable to start cursor thread");
        self.thread = Some(thread);
    }

    fn kill(&mut self) {
        if self.thread.is_none() {
            return;
        }
        thread::sleep(CURSOR_UPDATE_INTERVAL); // Give cursor some time to disable itself (cant kill while sleeping)
        self.thread.as_mut().unwrap().kill();
        self.thread = None;
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
            // CAUTION: This only works because LFBTerminal is the only implementation of Terminal
            let terminal = unsafe {
                (ptr::from_ref(self.terminal.as_ref()) as *const LFBTerminal)
                    .as_ref()
                    .unwrap()
            };

            // Disable cursor, if terminal is hidden
            if !terminal.display.lock().is_visible() {
                thread::switch();
                continue;
            }

            thread::sleep(CURSOR_UPDATE_INTERVAL);
            sleep_counter += CURSOR_UPDATE_INTERVAL;

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

            display.draw_direct_char(
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

impl CursorState {
    pub const fn new() -> Self {
        Self {
            pos: (0, 1),
            saved_pos: (0, 1),
        }
    }
}
