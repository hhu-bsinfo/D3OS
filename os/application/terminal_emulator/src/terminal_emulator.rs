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
use concurrent::thread::{self};
use core::usize;
use cursor::CursorThread;
use graphic::lfb::get_lfb_info;
use lfb_terminal::LFBTerminal;
use spin::Once;
use syscall::{SystemCall, syscall};
use terminal::Terminal;
use terminal_lib::{TerminalInputState, TerminalMode};

#[allow(unused_imports)]
use runtime::*;

const OUTPUT_BUFFER_SIZE: usize = 128;

static TERMINAL: Once<Arc<dyn Terminal>> = Once::new();

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
    thread::create(|| {
        let mut buffer: [u8; OUTPUT_BUFFER_SIZE] = [0; OUTPUT_BUFFER_SIZE];
        let terminal = terminal();

        loop {
            let result = syscall(
                SystemCall::TerminalReadOutput,
                &[buffer.as_mut_ptr() as usize, buffer.len()],
            );

            let byte_count = match result {
                Ok(0) => {
                    thread::switch();
                    continue;
                }
                Ok(count) => count,
                Err(_) => {
                    thread::switch();
                    continue;
                }
            };

            for byte in &mut buffer[0..byte_count] {
                terminal.write_byte(*byte);
                *byte = 0;
            }
        }
    });
}

fn observe_input() {
    thread::create(|| {
        let terminal = terminal();

        loop {
            let result = TerminalInputState::from(
                syscall(SystemCall::TerminalCheckInputState, &[]).unwrap() as usize,
            );

            let mode = match result {
                TerminalInputState::InputReaderAwaitsCooked => TerminalMode::Cooked,
                TerminalInputState::InputReaderAwaitsMixed => TerminalMode::Mixed,
                TerminalInputState::InputReaderAwaitsRaw => TerminalMode::Raw,
                TerminalInputState::Idle => {
                    thread::switch();
                    continue;
                }
            };

            let bytes = match terminal.read(mode) {
                Some(bytes) => bytes,
                None => vec![],
            };
            syscall(
                SystemCall::TerminalWriteInput,
                &[bytes.as_ptr() as usize, bytes.len(), mode as usize],
            );
        }
    });
}

fn start_text_session() {
    init_terminal(true);
    let terminal = terminal();
    terminal.clear();
    observe_input();
    observe_output();
    thread::start_application("shell", vec![]).unwrap().join();
}

fn start_window_manager_session() {
    init_terminal(false);
    observe_input();
    thread::start_application("window_manager", vec![])
        .unwrap()
        .join();
}

enum Session {
    Text,
    WindowManager,
}

#[unsafe(no_mangle)]
pub fn main() {
    let args = env::args();
    let mut session = Session::Text;

    for arg in args {
        match arg.as_str() {
            "--wm" => session = Session::WindowManager,
            _ => {}
        }
    }

    match session {
        Session::Text => start_text_session(),
        Session::WindowManager => start_window_manager_session(),
    }
}
