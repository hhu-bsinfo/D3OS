#![no_std]

extern crate alloc;
extern crate terminal as terminal_lib;

pub mod color;
pub mod cursor;
pub mod display;
pub mod lfb_terminal;
pub mod terminal;

use alloc::sync::Arc;
use alloc::vec;
use concurrent::thread::Thread;
use concurrent::thread::{self};
use cursor::start_cursor_thread;
use graphic::lfb::get_lfb_info;
use lfb_terminal::LFBTerminal;
use spin::Once;
use syscall::{SystemCall, syscall};
use terminal::Terminal;
use terminal_lib::{TerminalInputState, TerminalMode};

#[allow(unused_imports)]
use runtime::*;

const OUTPUT_BUFFER_SIZE: usize = 128;

static TERMINAL_EMULATOR: Once<TerminalEmulator> = Once::new();

pub struct TerminalEmulator {
    terminal: Arc<dyn Terminal>,
    cursor: Option<Thread>,
    operator: Option<Thread>,
}

impl TerminalEmulator {
    pub fn new(address: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) -> Self {
        let terminal = LFBTerminal::new(address, pitch, width, height, bpp, true);
        Self {
            terminal: Arc::new(terminal),
            cursor: None,
            operator: None,
        }
    }

    pub fn init(&mut self) {
        self.terminal().clear();
        self.cursor = start_cursor_thread();
        self.operator = thread::start_application("shell", vec![]);
    }

    pub fn terminal(&self) -> Arc<dyn Terminal> {
        Arc::clone(&self.terminal)
    }

    pub fn disable_visibility(&mut self) {
        self.terminal().hide();

        if self.operator.is_some() {
            let _ = syscall(SystemCall::TerminalTerminateOperator, &[1, 0]);
            self.operator = None;
        }

        thread::start_application("window_manager", vec![])
            .unwrap()
            .join();
        // Reenable visibility when window manager exits
        self.enable_visibility();
    }

    pub fn enable_visibility(&mut self) {
        self.terminal().show();

        if self.operator.is_none() {
            self.operator = Some(thread::start_application("shell", vec![]).unwrap());
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

    fn observe_input(&self) {
        let terminal = self.terminal();
        let result = TerminalInputState::from(
            syscall(SystemCall::TerminalCheckInputState, &[]).unwrap() as usize,
        );

        let mode = match result {
            TerminalInputState::InputReaderAwaitsCooked => TerminalMode::Cooked,
            TerminalInputState::InputReaderAwaitsMixed => TerminalMode::Mixed,
            TerminalInputState::InputReaderAwaitsRaw => TerminalMode::Raw,
            TerminalInputState::Idle => TerminalMode::Raw,
        };

        let bytes = match terminal.read(mode) {
            Some(bytes) => bytes,
            None => vec![],
        };

        if result == TerminalInputState::Idle {
            return;
        }

        syscall(
            SystemCall::TerminalWriteInput,
            &[bytes.as_ptr() as usize, bytes.len(), mode as usize],
        );
    }

    fn run(&self) {
        loop {
            self.observe_output();
            self.observe_input();
        }
    }
}

fn init_terminal_emulator() -> &'static TerminalEmulator {
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
        emulator
    })
}

pub fn terminal_emulator() -> &'static TerminalEmulator {
    TERMINAL_EMULATOR
        .get()
        .expect("Trying to access terminal emulator before initialization!")
}

#[unsafe(no_mangle)]
pub fn main() {
    let emulator = init_terminal_emulator();
    emulator.run()

    // terminal.clear();

    // terminal.write_str("Press 'F1' to toggle between text and gui mode\n\n");
}
