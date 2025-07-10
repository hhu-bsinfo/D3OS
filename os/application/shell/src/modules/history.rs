use alloc::{collections::vec_deque::VecDeque, string::String};
use terminal::{DecodedKey, KeyCode};

use crate::{
    context::context::Context,
    event::{
        event::Event,
        event_handler::{Error, EventHandler, Response},
    },
};

pub struct History {
    history: VecDeque<String>,
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
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
            history_position: -1,
        }
    }

    fn add(&mut self, clx: &mut Context) {
        self.history.push_front(clx.line.get().clone());
    }

    fn reset_position(&mut self) {
        self.history_position = -1
    }

    fn move_up(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if self.history_position >= self.history.len() as isize - 1 {
            return Ok(Response::Skip);
        }

        self.history_position += 1;
        self.restore(clx)
    }

    fn move_down(&mut self, clx: &mut Context) -> Result<Response, Error> {
        if self.history_position == -1 {
            return Ok(Response::Skip);
        }
        if self.history_position <= 0 {
            clx.line.reset();
            clx.events.trigger(Event::HistoryRestored);
            self.history_position = -1;
            return Ok(Response::Ok);
        }

        self.history_position -= 1;
        self.restore(clx)
    }

    fn restore(&mut self, clx: &mut Context) -> Result<Response, Error> {
        let line = self.history.get(self.history_position as usize).unwrap().clone();
        clx.line.reset();
        clx.line.push_str(&line);
        clx.line.set_cursor_pos(clx.line.len());
        clx.events.trigger(Event::HistoryRestored);
        Ok(Response::Ok)
    }
}
