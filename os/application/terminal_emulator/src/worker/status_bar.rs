use core::cell::RefCell;

use alloc::rc::Rc;
use time::systime;

use super::worker::Worker;
use crate::terminal::lfb_terminal::LFBTerminal;

const UPDATE_INTERVAL: i64 = 1000;

pub struct StatusBar {
    terminal: Rc<RefCell<LFBTerminal>>,
    last_tick: i64,
}

impl StatusBar {
    pub const fn new(terminal: Rc<RefCell<LFBTerminal>>) -> Self {
        Self {
            terminal,
            last_tick: 0,
        }
    }
}

impl Worker for StatusBar {
    fn run(&mut self) {
        let systime = systime().num_milliseconds();
        if systime < self.last_tick + UPDATE_INTERVAL {
            return;
        }
        self.last_tick = systime;
        LFBTerminal::draw_status_bar(&mut self.terminal.borrow().display.lock());
    }
}
