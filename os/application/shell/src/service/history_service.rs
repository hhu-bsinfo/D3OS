use alloc::collections::vec_deque::VecDeque;
use terminal::{DecodedKey, KeyCode};

use crate::context::Context;

use super::service::{Service, ServiceError};

pub struct HistoryService {
    history: VecDeque<Context>,
    history_position: isize,
}

impl Service for HistoryService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::RawKey(KeyCode::ArrowUp) => self.on_arrow_up(context),
            DecodedKey::RawKey(KeyCode::ArrowDown) => self.on_arrow_down(context),
            DecodedKey::Unicode('\n') => self.on_enter(context),
            _ => self.on_other_key(),
        }
    }
}

impl HistoryService {
    pub const fn new() -> Self {
        Self {
            history: VecDeque::new(),
            history_position: -1,
        }
    }

    fn on_arrow_up(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        if self.history_position == self.history.len() as isize - 1 {
            return Ok(());
        }

        self.move_history(context, 1)
    }

    fn on_arrow_down(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        if self.history_position <= -1 {
            return Ok(());
        }
        if self.history_position == 0 {
            self.history_position = -1;
            context.line.clear();
            context.set_dirty_offset_from_line(0);
            context.cursor_position = 0;
            return Ok(());
        }

        self.move_history(context, -1)
    }

    fn on_enter(&mut self, context: &mut Context) -> Result<(), ServiceError> {
        self.history.push_front(context.clone());
        self.history_position = -1;
        Ok(())
    }

    fn on_other_key(&mut self) -> Result<(), ServiceError> {
        self.history_position = -1;
        Ok(())
    }

    fn move_history(&mut self, context: &mut Context, step: isize) -> Result<(), ServiceError> {
        self.history_position += step;
        *context = self
            .history
            .get(self.history_position as usize)
            .unwrap()
            .clone();
        context.set_dirty_offset_from_line(0);

        Ok(())
    }
}
