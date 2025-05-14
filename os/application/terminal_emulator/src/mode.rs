use terminal_lib::TerminalMode;

pub struct Mode {
    current: TerminalMode,
}

impl Mode {
    pub const fn new(mode: TerminalMode) -> Self {
        Self { current: mode }
    }

    pub fn get(&self) -> TerminalMode {
        self.current
    }

    pub fn set(&mut self, mode: TerminalMode) {
        self.current = mode;
    }
}
