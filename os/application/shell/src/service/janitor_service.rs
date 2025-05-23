use terminal::{DecodedKey, print};

use crate::context::Context;

use super::service::{Service, ServiceError};

pub struct JanitorService {}

impl Service for JanitorService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.on_enter(context),
            _ => Ok(()),
        }
    }
}

impl JanitorService {
    pub const fn new() -> Self {
        Self {}
    }

    fn on_enter(&self, context: &mut Context) -> Result<(), ServiceError> {
        context.line.dirty_mut().clear();
        context.visual_line.dirty_mut().clear();
        context.cursor_position.set(0);
        context.tokens.dirty_mut().clear();
        context.cleanup();
        print!("\n");
        Ok(())
    }
}
