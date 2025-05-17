use alloc::{sync::Arc, vec};
use concurrent::thread::{self, Thread};
use syscall::{SystemCall, syscall};

use crate::{TerminalEmulator, terminal_emulator};

use super::worker::Worker;

pub struct OutputObserver {
    thread: Option<Thread>,
}

struct OutputObserverThread {
    emulator: Arc<TerminalEmulator>,
}

impl OutputObserver {
    pub const fn new() -> Self {
        Self { thread: None }
    }
}

impl Worker for OutputObserver {
    fn create(&mut self) {
        if self.thread.is_some() {
            return;
        }

        let thread = thread::create(|| {
            let mut observer = OutputObserverThread::new(terminal_emulator());
            observer.run();
        })
        .expect("Unable to start output observer thread");
        self.thread = Some(thread);
    }

    fn kill(&mut self) {
        if self.thread.is_none() {
            return;
        }
        self.thread.as_mut().unwrap().kill();
        self.thread = None;
    }
}

impl OutputObserverThread {
    pub const fn new(emulator: Arc<TerminalEmulator>) -> Self {
        Self { emulator }
    }

    fn run(&mut self) {
        let terminal = self.emulator.terminal();
        let mut buffer = vec![0u8; 128];

        loop {
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
}
