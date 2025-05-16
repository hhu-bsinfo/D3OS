#![no_std]

extern crate alloc;
extern crate terminal as terminal_lib;

pub mod color;
pub mod cursor;
pub mod decoder;
pub mod display;
pub mod event_handler;
pub mod lfb_terminal;
mod observer;
pub mod terminal;

use alloc::sync::Arc;
use alloc::vec;
use concurrent::thread::Thread;
use concurrent::thread::{self};
use cursor::start_cursor_thread;
use event_handler::{Event, EventHandler};
use graphic::lfb::get_lfb_info;
use lfb_terminal::LFBTerminal;
use observer::input_observer::start_input_observer_thread;
use spin::{Mutex, Once};
use syscall::{SystemCall, syscall};
use terminal::Terminal;

#[allow(unused_imports)]
use runtime::*;
use terminal_lib::write::log_debug;

const OUTPUT_BUFFER_SIZE: usize = 128;

static TERMINAL_EMULATOR: Once<Arc<TerminalEmulator>> = Once::new();

pub struct TerminalEmulator {
    terminal: Arc<dyn Terminal>,
    cursor: Mutex<Option<Thread>>,
    input_observer: Mutex<Option<Thread>>,
    operator: Mutex<Option<Thread>>,
    event_handler: Mutex<EventHandler>,
}

impl TerminalEmulator {
    pub fn new(address: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let terminal = LFBTerminal::new(address, pitch, width, height, bpp, true);
        Self {
            terminal: Arc::new(terminal),
            input_observer: Mutex::new(None),
            cursor: Mutex::new(None),
            operator: Mutex::new(None),
            event_handler: Mutex::new(EventHandler::new()),
        }
    }

    pub fn init(&mut self) {
        self.terminal().clear();
        *self.cursor.lock() = start_cursor_thread();
        *self.input_observer.lock() = start_input_observer_thread();
        *self.operator.lock() = thread::start_application("shell", vec![]);
    }

    pub fn terminal(&self) -> Arc<dyn Terminal> {
        Arc::clone(&self.terminal)
    }

    pub fn disable_visibility(&self) {
        self.terminal().hide();

        {
            let mut operator = self.operator.lock();
            if operator.is_some() {
                let _ = syscall(SystemCall::TerminalTerminateOperator, &[1, 0]);
                *operator = None;
            }
        }

        {
            let mut input_observer = self.input_observer.lock();
            if input_observer.is_some() {
                input_observer.as_mut().unwrap().kill();
                *input_observer = None;
            }
        }

        thread::start_application("window_manager", vec![])
            .unwrap()
            .join();

        // Reenable visibility when window manager exits
        self.enable_visibility();
    }

    pub fn enable_visibility(&self) {
        self.terminal().show();

        {
            let mut operator = self.operator.lock();
            log_debug("About to show");
            if operator.is_none() {
                *operator = Some(thread::start_application("shell", vec![]).unwrap());
            }
        }

        {
            let mut input_observer = self.input_observer.lock();
            if input_observer.is_none() {
                *input_observer = start_input_observer_thread();
            }
        }
    }

    fn observe_output(&self) {
        let mut buffer: [u8; OUTPUT_BUFFER_SIZE] = [0; OUTPUT_BUFFER_SIZE];
        let terminal = self.terminal();
        let result = syscall(
            SystemCall::TerminalReadOutput,
            &[buffer.as_mut_ptr() as usize, buffer.len()],
        );

        let byte_count = match result {
            Ok(0) => {
                return;
            }
            Ok(count) => count,
            Err(_) => {
                return;
            }
        };

        for byte in &mut buffer[0..byte_count] {
            terminal.write_byte(*byte);
            *byte = 0;
        }
    }

    fn run(&self) {
        loop {
            self.handle_events();
            self.observe_output();
        }
    }

    fn handle_events(&self) {
        let event = match self.event_handler.lock().handle() {
            Some(event) => event,
            None => return,
        };

        match event {
            Event::EnterGuiMode => self.disable_visibility(),
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
