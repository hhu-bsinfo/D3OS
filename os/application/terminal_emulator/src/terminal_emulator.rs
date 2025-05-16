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
use core::usize;
use cursor::CursorThread;
use graphic::lfb::get_lfb_info;
use lfb_terminal::LFBTerminal;
use spin::Once;
use spin::mutex::Mutex;
use syscall::{SystemCall, syscall};
use terminal::Terminal;
use terminal_lib::{TerminalInputState, TerminalMode};

#[allow(unused_imports)]
use runtime::*;

const OUTPUT_BUFFER_SIZE: usize = 128;

static TERMINAL: Once<Arc<dyn Terminal>> = Once::new();
static STATE: Once<Arc<Mutex<TerminalEmulator>>> = Once::new();

pub struct TerminalEmulator {
    operator: Option<Thread>,
}

impl TerminalEmulator {
    pub const fn new() -> Self {
        Self { operator: None }
    }

    pub fn init(&mut self) {
        let shell = thread::start_application("shell", vec![]).unwrap();
        self.operator = Some(shell);
    }

    pub fn disable_visibility(&mut self) {
        terminal().hide();

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
        terminal().show();

        if self.operator.is_none() {
            self.operator = Some(thread::start_application("shell", vec![]).unwrap());
        }
    }
}

fn init_terminal_emulator() {
    STATE.call_once(|| Arc::new(Mutex::new(TerminalEmulator::new())));
}

pub fn terminal_emulator() -> Arc<Mutex<TerminalEmulator>> {
    STATE.get().unwrap().clone()
}

pub fn init_terminal(visible: bool) {
    let lfb_info = get_lfb_info();
    let lfb_terminal = Arc::new(LFBTerminal::new(
        lfb_info.address as *mut u8,
        lfb_info.pitch,
        lfb_info.width,
        lfb_info.height,
        lfb_info.bpp,
        visible,
    ));
    lfb_terminal.clear();
    TERMINAL.call_once(|| lfb_terminal);

    thread::create(|| {
        let mut cursor_thread = CursorThread::new(terminal());
        cursor_thread.run();
    });
}

pub fn terminal() -> Arc<dyn Terminal> {
    let terminal = TERMINAL
        .get()
        .expect("Trying to access terminal before initialization!");
    Arc::clone(terminal)
}

fn observe_output() {
    let mut buffer: [u8; OUTPUT_BUFFER_SIZE] = [0; OUTPUT_BUFFER_SIZE];
    let terminal = terminal();
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

fn observe_input() {
    let terminal = terminal();
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

fn run() {
    loop {
        observe_output();
        observe_input();
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    init_terminal_emulator();
    init_terminal(true);
    let terminal_emulator = terminal_emulator();
    terminal_emulator.lock().init();
    let terminal = terminal();

    terminal.clear();

    terminal.write_str("Press 'F1' to toggle between text and gui mode\n\n");

    run()
}
