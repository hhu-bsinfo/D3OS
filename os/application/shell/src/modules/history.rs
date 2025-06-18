use alloc::collections::vec_deque::VecDeque;
use terminal::{DecodedKey, KeyCode};

use crate::{
    context::context::Context,
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
};

pub struct History {
    history: VecDeque<Context>,
    history_position: isize,
}

impl EventHandler for History {
    fn on_key_pressed(&mut self, clx: &mut Context, key: DecodedKey) -> Result<Response, Error> {
        match key {
            DecodedKey::RawKey(KeyCode::ArrowUp) => self.move_up(clx),
            DecodedKey::RawKey(KeyCode::ArrowDown) => self.move_down(clx),
            _ => Ok(Response::Skip),
        }
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.reset_position();
        self.add(clx);
        Ok(Response::Ok)
    }

    fn on_line_written(&mut self, _clx: &mut Context) -> Result<Response, Error> {
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
        clx.events.trigger(Event::HistoryRestored);
        Ok(Response::Ok)
    }
}
