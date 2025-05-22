use core::cell::RefCell;

use alloc::{rc::Rc, vec};
use stream::OutputStream;
use syscall::{SystemCall, syscall};

use crate::terminal::lfb_terminal::LFBTerminal;

use super::worker::Worker;

pub struct OutputObserver {
    terminal: Rc<RefCell<LFBTerminal>>,
}

impl OutputObserver {
    pub const fn new(terminal: Rc<RefCell<LFBTerminal>>) -> Self {
        Self { terminal }
    }
}

impl Worker for OutputObserver {
    fn run(&mut self) {
        let terminal = self.terminal.borrow();
        let mut buffer = vec![0u8; 128];

        let read_bytes = syscall(
            SystemCall::TerminalReadOutput,
            &[buffer.as_mut_ptr() as usize, buffer.len()],
        )
        .expect("Unable to read output from tty");

        for byte in &mut buffer[0..read_bytes] {
            terminal.write_byte(*byte);
            *byte = 0;
        }
    }
}
