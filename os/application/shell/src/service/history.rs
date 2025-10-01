use alloc::{collections::vec_deque::VecDeque, string::String};
use terminal::{DecodedKey, KeyCode};

use crate::{
    context::{context::ContextProvider, line_context::LineContext},
    event::{
        event::Event,
        event_bus::EventBus,
        event_handler::{Error, EventHandler, Response},
    },
};

const MAX_HISTORY_LEN: usize = 10;

pub struct HistoryService {
    line_provider: ContextProvider<LineContext>,

    history: VecDeque<String>,
    history_position: isize,
}

impl EventHandler for HistoryService {
    fn on_key_pressed(&mut self, event_bus: &mut EventBus, key: DecodedKey) -> Result<Response, Error> {
        match key {
            DecodedKey::RawKey(KeyCode::ArrowUp) => self.move_up(event_bus),
            DecodedKey::RawKey(KeyCode::ArrowDown) => self.move_down(event_bus),
            _ => Ok(Response::Skip),
        }
    }

    fn on_submit(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.reset_position();
        self.add();
        Ok(Response::Ok)
    }

    fn on_line_written(&mut self, _event_bus: &mut EventBus) -> Result<Response, Error> {
        self.reset_position();
        Ok(Response::Ok)
    }
}

impl HistoryService {
    pub fn new(line_provider: ContextProvider<LineContext>) -> Self {
        Self {
            line_provider,
            history: VecDeque::new(),
            history_position: -1,
        }
    }

    fn add(&mut self) {
        if self.history.len() >= MAX_HISTORY_LEN {
            self.history.pop_back();
        }

        let line_clx = self.line_provider.borrow();
        self.history.push_front(line_clx.get().clone());
    }

    fn reset_position(&mut self) {
        self.history_position = -1
    }

    fn move_up(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        if self.history_position >= self.history.len() as isize - 1 {
            return Ok(Response::Skip);
        }

        self.history_position += 1;
        self.restore(event_bus)
    }

    fn move_down(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        if self.history_position == -1 {
            return Ok(Response::Skip);
        }
        if self.history_position <= 0 {
            self.line_provider.borrow_mut().reset();
            event_bus.trigger(Event::HistoryRestored);
            self.history_position = -1;
            return Ok(Response::Ok);
        }

        self.history_position -= 1;
        self.restore(event_bus)
    }

    fn restore(&mut self, event_bus: &mut EventBus) -> Result<Response, Error> {
        let mut line_clx = self.line_provider.borrow_mut();
        let line = self.history.get(self.history_position as usize).unwrap().clone();
        line_clx.reset();
        line_clx.push_str(&line);
        line_clx.set_cursor_pos(line.len());
        event_bus.trigger(Event::HistoryRestored);
        Ok(Response::Ok)
    }
}
