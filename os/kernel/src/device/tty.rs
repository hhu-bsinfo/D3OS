use core::{
    sync::atomic::{AtomicUsize, Ordering},
};

use alloc::collections::vec_deque::VecDeque;
use num_enum::{FromPrimitive, IntoPrimitive};
use spin::Mutex;
use terminal::TerminalMode;
use crate::scheduler;

/// TTY-Input device (Workaround for missing pipes).
/// Buffers input from the terminal when an application is reading.
/// Depending on the chosen TerminalMode, the application will block until terminal has written some input.
///
/// NOTE: Just a Workaround. Parallel reads where not considered and might cause problems.
///
/// Author: Sebastian Keller
#[derive(Debug)]
pub struct TtyInput {
    buffer: Mutex<VecDeque<u8>>,
    state: AtomicUsize,
    mode: AtomicUsize,
}

/// TTY-Output device (Workaround for missing pipes).
/// Buffers output from from applications.
/// Applications don't block and can continue after writing.
/// The terminal collects the data from the buffer if present.
///
/// NOTE: Just a Workaround. Parallel writes where not considered and might cause problems.
/// NOTE: The application will write faster then the terminal can collect.
///       Writing a extreme amount of data will cause an out-of-memory error.
///
/// Author: Sebastian Keller
#[derive(Debug)]
pub struct TtyOutput {
    buffer: Mutex<VecDeque<u8>>,
}

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive, Clone, Copy)]
#[repr(usize)]
pub enum TtyInputState {
    #[num_enum(default)]
    Idle = 0,
    Waiting = 1,
    Ready = 2,
}

impl TtyInput {
    pub const fn new() -> Self {
        TtyInput {
            buffer: Mutex::new(VecDeque::new()),
            state: AtomicUsize::new(TtyInputState::Idle as usize),
            mode: AtomicUsize::new(TerminalMode::Canonical as usize),
        }
    }

    pub fn read(&self, buffer: &mut [u8], mode: TerminalMode) -> usize {
        self.state.store(TtyInputState::Waiting as usize, Ordering::SeqCst);
        self.mode.store(mode.into(), Ordering::SeqCst);

        while self.state.load(Ordering::SeqCst) != (TtyInputState::Ready as usize) {
            scheduler().switch_thread_no_interrupt();
        }

        let mut input_buffer = self.buffer.lock();
        let mut count = 0;
        for byte in buffer {
            *byte = match input_buffer.pop_front() {
                Some(byte) => byte,
                None => break,
            };
            count += 1;
        }

        self.state.store(TtyInputState::Idle as usize, Ordering::SeqCst);

        count
    }

    pub fn write(&self, bytes: &[u8], mode: TerminalMode) -> usize {
        if self.state.load(Ordering::SeqCst) != (TtyInputState::Waiting as usize) {
            return 0; // Abort, no more readers
        }
        if self.mode.load(Ordering::SeqCst) != mode as usize {
            return 0; //Abort, mismatched mode
        }

        let mut input_buffer = self.buffer.lock();
        let mut count = 0;
        for byte in bytes {
            input_buffer.push_back(*byte);
            count += 1;
        }

        self.state.store(TtyInputState::Ready as usize, Ordering::SeqCst);

        count
    }

    pub fn state(&self) -> TtyInputState {
        TtyInputState::from(self.state.load(Ordering::SeqCst))
    }

    pub fn mode(&self) -> TerminalMode {
        TerminalMode::from(self.mode.load(Ordering::SeqCst))
    }
}

impl TtyOutput {
    pub const fn new() -> Self {
        Self {
            buffer: Mutex::new(VecDeque::new()),
        }
    }

    pub fn write(&self, bytes: &[u8]) -> usize {
        let mut output_buffer = self.buffer.lock();
        let mut count = 0;
        for byte in bytes {
            output_buffer.push_back(*byte);
            count += 1;
        }

        count
    }

    pub fn read(&self, buffer: &mut [u8]) -> usize {
        let mut output_buffer = self.buffer.lock();
        let mut count = 0;
        for byte in buffer {
            *byte = match output_buffer.pop_front() {
                Some(read_byte) => {
                    count += 1;
                    read_byte
                }
                None => break,
            };
        }

        count
    }
}
