use alloc::collections::vec_deque::VecDeque;

use crate::context::Context;

use super::service::{Error, Response, Service};

pub struct HistoryService {
    history: VecDeque<Context>,
    history_position: isize,
}

impl Service for HistoryService {
    fn history_up(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.move_up(context)
    }

    fn history_down(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.move_down(context)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.reset_position();
        self.add(context);
        Ok(Response::Ok)
    }

    fn simple_key(&mut self, _context: &mut Context, _key: char) -> Result<Response, Error> {
        self.reset_position();
        Ok(Response::Ok)
    }
}

impl HistoryService {
    pub const fn new() -> Self {
        Self {
            history: VecDeque::new(),
            history_position: -1,
        }
    }

    fn add(&mut self, context: &mut Context) {
        self.history.push_front(context.clone());
    }

    fn reset_position(&mut self) {
        self.history_position = -1
    }

    fn move_up(&mut self, context: &mut Context) -> Result<Response, Error> {
        if self.history_position == self.history.len() as isize - 1 {
            return Ok(Response::Skip);
        }

        self.move_by(context, 1)
    }

    fn move_down(&mut self, context: &mut Context) -> Result<Response, Error> {
        if self.history_position <= -1 {
            return Ok(Response::Skip);
        }
        if self.history_position == 0 {
            self.history_position = -1;
            context.line.reset();
            context.cursor_position = 0;
            return Ok(Response::Ok);
        }

        self.move_by(context, -1)
    }

    fn move_by(&mut self, context: &mut Context, step: isize) -> Result<Response, Error> {
        self.history_position += step;
        *context = self
            .history
            .get(self.history_position as usize)
            .unwrap()
            .clone();
        context.line.mark_dirty_at(0);

        Ok(Response::Ok)
    }
}
