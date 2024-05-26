use alloc::boxed::Box;
use drawer::drawer::{RectData, Vertex};

use crate::{api::Command, API};

pub struct TestComp {
    handle: usize,
}

impl TestComp {
    pub fn new(handle: usize) -> Self {
        Self {
            handle,
        }
    }

    pub fn run(&mut self) {
        let api = unsafe { API.get_mut().unwrap().lock() };
        let mut qwe = 0;
        api.execute(
            self.handle, 
            Command::CreateButton { 
                pos: RectData { 
                    top_left: Vertex::new(20, 20),
                    width: 40,
                    height: 30,
                }, 
                label: Some("FELS"),
                //TODO: Figure out how to include closures, think of gtk-rs
                on_click: Box::new(move || { qwe += 1; }),
            },
        );
    }
}