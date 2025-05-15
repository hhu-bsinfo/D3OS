#![no_std]

extern crate alloc;
extern crate terminal as terminal_lib;

pub mod color;
pub mod cursor;
pub mod display;
pub mod lfb_terminal;
pub mod terminal;

use alloc::{format, vec};
use alloc::{string::String, sync::Arc};
use concurrent::thread::{self};
use core::usize;
use cursor::CursorThread;
use graphic::lfb::get_lfb_info;
use lfb_terminal::LFBTerminal;
use pc_keyboard::{DecodedKey, KeyCode};
use spin::Once;
use syscall::{SystemCall, syscall};
use terminal::Terminal;
use terminal_lib::{DecodedKeyType, TerminalInputState, TerminalMode};

#[allow(unused_imports)]
use runtime::*;

const OUTPUT_BUFFER_SIZE: usize = 128;

static TERMINAL: Once<Arc<dyn Terminal>> = Once::new();

pub fn init_terminal() {
    let lfb_info = get_lfb_info();
    let lfb_terminal = Arc::new(LFBTerminal::new(
        lfb_info.address as *mut u8,
        lfb_info.pitch,
        lfb_info.width,
        lfb_info.height,
        lfb_info.bpp,
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
            let result = syscall(SystemCall::TerminalInputState, &[]).unwrap() as usize;

            if TerminalInputState::from(result) == TerminalInputState::Idle {
                thread::switch();
                continue;
            }

            match terminal.read_byte() {
                ..0 => continue,
                byte => syscall(SystemCall::TerminalWriteInput, &[byte as usize]),
            };
        }
    });
}

#[unsafe(no_mangle)]
pub fn main() {
    init_terminal();
    let terminal = terminal();
    terminal.clear();

    // observe_output();
    // observe_input();

    // thread::start_application("shell", vec![]);

    loop {
        // COOKED
        terminal.write_str("Enter cooked line:\n");
        let bytes = terminal.read(TerminalMode::Cooked);
        let string = String::from_utf8(bytes).unwrap();
        terminal.write_str(&format!("Received: {}\n\n", string));

        // // MIXED
        // terminal.write_str("Enter key (mixed):\n");
        // let bytes = terminal.read(TerminalMode::Mixed);
        // let key_type = match bytes.first() {
        //     Some(k) => *k,
        //     None => continue,
        // };
        // let key = match bytes.last() {
        //     Some(k) => *k,
        //     None => continue,
        // };
        // if DecodedKeyType::from(key_type) == DecodedKeyType::RawKey {
        //     terminal.write_str(&format!(
        //         "Received RawKey: {:?}\n\n",
        //         DecodedKey::RawKey(unsafe { core::mem::transmute(key) })
        //     ));
        // };
        // if DecodedKeyType::from(key_type) == DecodedKeyType::Unicode {
        //     terminal.write_str(&format!(
        //         "Received Unicode: {:?}\n\n",
        //         DecodedKey::Unicode(key as char)
        //     ));
        // };

        // // RAW
        // terminal.write_str("Enter key (raw):\n");
        // let bytes = terminal.read(TerminalMode::Raw);
        // let key = match bytes.first() {
        //     Some(k) => *k,
        //     None => continue,
        // };
        // terminal.write_str(&format!("Received Raw key: {:?}\n\n", key));
    }
}
