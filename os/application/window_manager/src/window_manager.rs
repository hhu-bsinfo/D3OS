#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use alloc::boxed::Box;
use drawer::drawer::{Drawer, Vertex};
#[allow(unused_imports)]
use runtime::*;

struct WindowManager {
    root_window: RootWindow,
    drawer: Drawer,
}

/// `RootWindow` always fills out the entire available screen-space
struct RootWindow {
    children: Vec<CompositeWindow>,
}

/// `pos_in_parent` uses the position of the top-left vertex
struct CompositeWindow {
    pos_in_parent: (u32, u32),
    children: Vec<CompositeWindow>,
}

impl WindowManager {
    fn new() -> WindowManager {
        Self {
            root_window: RootWindow::new(),
            drawer: Drawer::new(),
        }
    }
}

impl RootWindow {
    fn new() -> RootWindow { 
        Self { children: Vec::new() }
    }
}

impl CompositeWindow {
    fn new(pos_in_parent: (u32, u32)) -> CompositeWindow { 
        Self { pos_in_parent, children: Vec::new() }
    }

    fn close(&self) {
        todo!()
    }

    fn translate(&self) {
        todo!()
    }

    fn minimize(&self) {
        todo!()
    }
}

#[no_mangle]
pub fn main() {
    WindowManager::new();
}