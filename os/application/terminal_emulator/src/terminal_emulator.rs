#![no_std]

extern crate alloc;
extern crate terminal as terminal_lib;

mod event_handler;
mod sessions;
mod terminal;
mod util;
mod worker;

use core::cell::RefCell;

use alloc::rc::Rc;
use alloc::vec;
use concurrent::thread::{self, sleep};
use event_handler::{Event, EventHandler};
use graphic::lfb::map_framebuffer;
use sessions::SessionMux;
use terminal::lfb_terminal::LFBTerminal;
use logger::init_logger;
use worker::cursor::Cursor;
use worker::input_observer::InputObserver;

#[allow(unused_imports)]
use runtime::*;
use worker::status_bar::StatusBar;
use worker::worker::Worker;

/// This application emulates the terminal.
/// The kernel lfb_terminal device has been migrated here and is mostly unchanged.
///
/// Special operations like IO, Cursor or status bar are managed in individual worker objects.
/// Input is handled by `InputObserver`; output/session multiplexing is handled by `SessionMux`.
///
/// The terminal is running single threaded but has been structured to support multi threading in future if needed.
///
/// Authors: Sebastian Keller, Aymen Sellami
pub struct TerminalEmulator {
    terminal: Rc<LFBTerminal>,
    event_handler: Rc<RefCell<EventHandler>>,
    mux: Rc<RefCell<SessionMux>>,
    input_observer: InputObserver,
    cursor: Cursor,
    status_bar: StatusBar,
}

impl TerminalEmulator {
    pub fn new(address: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let terminal = Rc::new(LFBTerminal::new(address, pitch, width, height, bpp));
        let event_handler = Rc::new(RefCell::new(EventHandler::new()));
        let mux = Rc::new(RefCell::new(SessionMux::new(terminal.clone())));
        Self {
            terminal: terminal.clone(),
            input_observer: InputObserver::new(terminal.clone(), event_handler.clone(), mux.clone()),
            cursor: Cursor::new(terminal.clone()),
            event_handler,
            mux,
            status_bar: StatusBar::new(terminal),
        }
    }

    pub fn enter_gui(&self) {
        {
            let mut display = self.terminal.display.lock();
            display.lfb.direct_lfb().draw_loader();
            let _ = thread::start_application("window_manager", vec![]).unwrap().join(); // Wait for window manager to exit, then continue
            display.lfb.direct_lfb().draw_loader();
            sleep(500); // Solves an issue where sometimes workspaces from the window manager are still visible when toggling quickly between text and gui
            display.lfb.flush();
        }
        // Repaint the active session over the loader screen once we are back.
        self.mux.borrow().present_active();
    }

    fn run(&mut self) {
        loop {
            self.handle_events();
            {
                let mut mux = self.mux.borrow_mut();
                mux.poll_snapshot();
                mux.drain_outputs();
            }
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
    init_logger();
    let lfb_info = map_framebuffer().expect("failed to get framebuffer");
    sessions::manager::init();
    let mut emulator = TerminalEmulator::new(
        lfb_info.addr as *mut u8,
        lfb_info.pitch,
        lfb_info.width,
        lfb_info.height,
        lfb_info.bpp,
    );
    emulator.run()
}
