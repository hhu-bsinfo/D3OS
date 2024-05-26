use drawer::drawer::Vertex;

use crate::{api::Command, API};

pub(in super::super) struct TestComp {
    handle: usize,
}

impl TestComp {
    pub(in super::super) fn new(handle: usize) -> Self {
        Self {
            handle,
        }
    }

    pub(in super::super) fn run(&mut self) {
        let api = unsafe { API.get_mut().unwrap().lock() };
        api.draw(
            self.handle, 
            Command::DrawRectangle { 
                top_left: Vertex::new(400, 400), 
                width: 300, 
                height: 200, 
            },
        );
    }
}