#![no_std]

extern crate alloc;
extern crate terminal as terminal_lib;

pub mod color;
pub mod cursor;
pub mod decoder;
pub mod display;
pub mod event_handler;
pub mod lfb_terminal;
pub mod terminal;
mod worker;

use alloc::sync::Arc;
use alloc::vec;
use concurrent::thread::Thread;
use concurrent::thread::{self};
use cursor::start_cursor_thread;
use event_handler::{Event, EventHandler};
use graphic::lfb::get_lfb_info;
use lfb_terminal::LFBTerminal;
use spin::{Mutex, Once};
use terminal::Terminal;
use worker::input_observer::InputObserver;

#[allow(unused_imports)]
use runtime::*;
use worker::operator::Operator;
use worker::output_observer::OutputObserver;
use worker::worker::Worker;

static TERMINAL_EMULATOR: Once<Arc<TerminalEmulator>> = Once::new();

pub struct TerminalEmulator {
    terminal: Arc<dyn Terminal>,
    cursor: Mutex<Option<Thread>>,
    input_observer: Mutex<InputObserver>,
    output_observer: Mutex<OutputObserver>,
    operator: Mutex<Operator>,
    event_handler: Mutex<EventHandler>,
}

impl TerminalEmulator {
    pub fn new(address: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let terminal = LFBTerminal::new(address, pitch, width, height, bpp, true);
        Self {
            terminal: Arc::new(terminal),
            input_observer: Mutex::new(InputObserver::new()),
            output_observer: Mutex::new(OutputObserver::new()),
            cursor: Mutex::new(None),
            operator: Mutex::new(Operator::new()),
            event_handler: Mutex::new(EventHandler::new()),
        }
    }

    pub fn init(&mut self) {
        self.terminal().clear();
        *self.cursor.lock() = start_cursor_thread();
        self.input_observer.lock().create();
        self.output_observer.lock().create();
        self.operator.lock().create();
    }

    pub fn terminal(&self) -> Arc<dyn Terminal> {
        Arc::clone(&self.terminal)
    }

    pub fn disable(&self) {
        self.terminal().hide();
        {
            /* Separate block, because lock would extend into self.enable() causing infinite lock */
            self.operator.lock().kill();
            self.input_observer.lock().kill();
            self.output_observer.lock().kill();
        }

        // Reenable visibility when window manager exits
        thread::start_application("window_manager", vec![])
            .unwrap()
            .join();
        self.enable();
    }

    pub fn enable(&self) {
        self.terminal().show();
        self.operator.lock().create();
        self.input_observer.lock().create();
        self.output_observer.lock().create();
    }

    fn run(&self) {
        loop {
            self.handle_events();
        }
    }

    fn handle_events(&self) {
        let event = match self.event_handler.lock().handle() {
            Some(event) => event,
            None => return,
        };

        match event {
            Event::EnterGuiMode => self.disable(),
        }

        self.handle_events();
    }
}

fn init_terminal_emulator() {
    TERMINAL_EMULATOR.call_once(|| {
        let lfb_info = get_lfb_info();
        let mut emulator = TerminalEmulator::new(
            lfb_info.address as *mut u8,
            lfb_info.pitch,
            lfb_info.width,
            lfb_info.height,
            lfb_info.bpp,
        );
        emulator.init();
        Arc::new(emulator)
    });
}

pub fn terminal_emulator() -> Arc<TerminalEmulator> {
    let emulator = TERMINAL_EMULATOR
        .get()
        .expect("Trying to access terminal emulator before initialization!");
    Arc::clone(emulator)
}

#[unsafe(no_mangle)]
pub fn main() {
    init_terminal_emulator();
    let emulator = terminal_emulator();
    emulator.run()

    // terminal.write_str("Press 'F1' to toggle between text and gui mode\n\n");
}
