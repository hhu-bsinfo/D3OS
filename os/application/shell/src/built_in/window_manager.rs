use terminal::{print, println};

use crate::built_in::built_in::BuiltIn;

pub struct WindowManagerBuiltIn {}

impl BuiltIn for WindowManagerBuiltIn {
    fn namespace(&self) -> &'static str {
        "window_manager"
    }

    fn run(&mut self, _args: &[&str]) -> isize {
        println!("Press F1 to start window manager");
        -1
    }
}

impl WindowManagerBuiltIn {
    pub fn new() -> Self {
        Self {}
    }
}
