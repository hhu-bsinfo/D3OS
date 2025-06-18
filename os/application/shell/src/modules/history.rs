use alloc::collections::vec_deque::VecDeque;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
};

pub struct History {
    history: VecDeque<Context>,
    history_position: isize,
}

impl EventHandler for History {
    fn history_up(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.move_up(clx)
    }

    fn history_down(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.move_down(clx)
    }

    fn submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.reset_position();
        self.add(clx);
        Ok(Response::Ok)
    }

    fn simple_key(&mut self, _clx: &mut Context, _key: char) -> Result<Response, Error> {
        self.reset_position();
        Ok(Response::Ok)
    }
}

impl History {
    pub const fn new() -> Self {
        Self {
            history: VecDeque::new(),
            history_position: -1,
        }
    }

    fn add(&mut self, clx: &mut Context) {
        self.history.push_front(clx.clone());
    }

    fn reset_position(&mut self) {
        self.history_position = -1
    }

    fn move_up(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if self.history_position == self.history.len() as isize - 1 {
            return Ok(Response::Skip);
        }

        self.move_by(clx, 1)
    }

    fn move_down(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if self.history_position <= -1 {
            return Ok(Response::Skip);
        }
        if self.history_position == 0 {
            self.history_position = -1;
            clx.line.reset();
            return Ok(Response::Ok);
        }

        self.move_by(clx, -1)
    }

    fn move_by(&mut self, clx: &mut Context, step: isize) -> Result<Response, Error> {
        self.history_position += step;
        *clx = self.history.get(self.history_position as usize).unwrap().clone();
        clx.line.mark_dirty_at(0);

        Ok(Response::Ok)
    }
}
