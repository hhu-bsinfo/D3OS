use terminal::DecodedKey;

use crate::context::Context;

use super::service::{Service, ServiceError};

pub struct JanitorService {}

impl Service for JanitorService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.on_enter(context),
            _ => self.on_other_key(context),
        }
    }
}

impl JanitorService {
    pub const fn new() -> Self {
        Self {}
    }

    fn on_other_key(&self, context: &mut Context) -> Result<(), ServiceError> {
        context.dirty_offset = context.total_line_len();

        Ok(())
    }

    fn on_enter(&self, context: &mut Context) -> Result<(), ServiceError> {
        context.line.clear();
        context.line_prefix.clear();
        context.line_suffix.clear();
        context.tokens.clear();
        context.cursor_position = 0;
        context.dirty_offset = 0;
        Ok(())
    }
}
