use alloc::vec::Vec;
use terminal::{print, println};

pub struct WindowManagerBuildIn {}

impl WindowManagerBuildIn {
    pub fn new(_args: Vec<&str>) -> Self {
        Self {}
    }

    pub fn start(&self) -> Result<(), ()> {
        println!("Press F1 to start window manager");
        Err(())
    }
}
