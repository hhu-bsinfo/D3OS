use alloc::rc::Rc;
use core::cell::RefCell;
use ime::pinyin::PinyinClient;
use time::systime;

use super::worker::Worker;
use crate::terminal::lfb_terminal::LFBTerminal;

const UPDATE_INTERVAL: i64 = 1000;
const LAYOUT_POLL_INTERVAL: i64 = 50;

pub struct StatusBar {
    terminal: Rc<LFBTerminal>,
    pinyin: Rc<RefCell<PinyinClient>>,
    last_tick: i64,
    last_poll: i64,
    last_layout: Option<ime::Layout>,
}

impl StatusBar {
    pub const fn new(terminal: Rc<LFBTerminal>, pinyin: Rc<RefCell<PinyinClient>>) -> Self {
        Self {
            terminal,
            pinyin,
            last_tick: -UPDATE_INTERVAL,
            last_poll: -LAYOUT_POLL_INTERVAL,
            last_layout: None,
        }
    }
}

impl Worker for StatusBar {
    fn run(&mut self) {
        let systime = systime().num_milliseconds();
        if systime < self.last_poll + LAYOUT_POLL_INTERVAL {
            return;
        }
        self.last_poll = systime;
        let layout = ime::get_layout().ok();
        let mut pinyin = self.pinyin.borrow_mut();
        pinyin.sync_layout(layout);
        let status = pinyin.status();
        let mut display = self.terminal.display.lock();
        if layout != self.last_layout || display.ime_status != status || systime >= self.last_tick + UPDATE_INTERVAL {
            self.last_tick = systime;
            self.last_layout = layout;
            display.ime_status.clear();
            display.ime_status.push_str(status);
            LFBTerminal::draw_status_bar(&mut display, layout);
        }
    }
}
