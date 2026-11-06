use alloc::rc::Rc;
use time::systime;

use crate::terminal::lfb_terminal::LFBTerminal;

use super::worker::Worker;

const UPDATE_INTERVAL: i64 = 250;

pub struct Cursor {
    terminal: Rc<LFBTerminal>,
    last_tick: i64,
}

impl Cursor {
    pub fn new(terminal: Rc<LFBTerminal>) -> Self {
        Self {
            terminal,
            last_tick: -UPDATE_INTERVAL,
        }
    }
}

impl Worker for Cursor {
    fn run(&mut self) {
        let systime = systime().num_milliseconds();

        if systime < self.last_tick + UPDATE_INTERVAL {
            return;
        }
        self.last_tick = systime;

        self.terminal.toggle_cursor();
    }
}
