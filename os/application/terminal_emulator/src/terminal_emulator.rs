#![no_std]

extern crate alloc;
extern crate terminal as terminal_lib;

pub mod event_handler;
mod operator;
pub mod terminal;
pub mod util;
mod worker;

use core::cell::RefCell;

use alloc::rc::Rc;
use alloc::vec;
use concurrent::thread::{self, sleep};
use event_handler::{Event, EventHandler};
use graphic::lfb::get_lfb_info;
use operator::Operator;
use stream::OutputStream;
use terminal::lfb_terminal::LFBTerminal;
use util::banner::create_banner_string;
use worker::cursor::Cursor;
use worker::input_observer::InputObserver;

#[allow(unused_imports)]
use runtime::*;
use worker::output_observer::OutputObserver;
use worker::status_bar::StatusBar;
use worker::worker::Worker;

pub struct TerminalEmulator {
    terminal: Rc<LFBTerminal>,
    event_handler: Rc<RefCell<EventHandler>>,
    input_observer: InputObserver,
    output_observer: OutputObserver,
    cursor: Cursor,
    operator: Operator,
    status_bar: StatusBar,
}

impl TerminalEmulator {
    pub fn new(address: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let terminal = Rc::new(LFBTerminal::new(address, pitch, width, height, bpp));
        let event_handler = Rc::new(RefCell::new(EventHandler::new()));
        Self {
            terminal: terminal.clone(),
            input_observer: InputObserver::new(terminal.clone(), event_handler.clone()),
            output_observer: OutputObserver::new(terminal.clone()),
            cursor: Cursor::new(terminal.clone()),
            operator: Operator::new(),
            event_handler: event_handler,
            status_bar: StatusBar::new(terminal),
        }
    }

    pub fn init(&mut self) {
        self.terminal.write_str(&create_banner_string()); // TODO move into terminal to be able to redraw after clearing screen
        self.operator.create();
    }

    pub fn enter_gui(&self) {
        let mut display = self.terminal.display.lock();
        display.lfb.direct_lfb().draw_loader();
        thread::start_application("window_manager", vec![]).unwrap().join(); // Wait for window manager to exit, then continue
        display.lfb.direct_lfb().draw_loader();
        sleep(500); // Solves an issue where sometimes workspaces from window manager are still visible when toggling quickly between text and gui
        display.lfb.flush();
    }

    fn run(&mut self) {
        loop {
            self.handle_events();
            self.output_observer.run();
            self.input_observer.run();
            self.cursor.run();
            self.status_bar.run();
        }
    }

    fn handle_events(&self) {
        let event = match self.event_handler.borrow_mut().handle() {
            Some(event) => event,
            None => return,
        };

        match event {
            Event::EnterGuiMode => self.enter_gui(),
        }

        self.handle_events();
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    let lfb_info = get_lfb_info();
    let mut emulator = TerminalEmulator::new(
        lfb_info.address as *mut u8,
        lfb_info.pitch,
        lfb_info.width,
        lfb_info.height,
        lfb_info.bpp,
    );
    emulator.init();
    emulator.run()
}

// TODO#9 BUG: terminal won't print just parentheses (Example ['''''''] => [], [''''''] => [], ["'"'"'"] => [], but [a'b'c"d"e] => [a'b'c"d"e])
