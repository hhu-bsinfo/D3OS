use crate::context::Context;

use super::service::{Error, Response, Service};

pub struct JanitorService {}

/// TODO Remove this service (handle in own services with prepare) CURRENT BUG: JANITOR WONT RUN ON ERROR
impl Service for JanitorService {
    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.handle_submit(context)
    }

    fn history_down(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.handle_other_key(context)
    }

    fn history_up(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.handle_other_key(context)
    }

    fn simple_key(&mut self, context: &mut Context, _key: char) -> Result<Response, Error> {
        self.handle_other_key(context)
    }
}

impl JanitorService {
    pub const fn new() -> Self {
        Self {}
    }

    fn handle_other_key(&self, context: &mut Context) -> Result<Response, Error> {
        context.dirty_offset = context.total_line_len();
        Ok(Response::Ok)
    }

    fn handle_submit(&self, context: &mut Context) -> Result<Response, Error> {
        context.line.clear();
        context.line_prefix.clear();
        context.line_suffix.clear();
        context.tokens.clear();
        context.executable = None;
        context.cursor_position = 0;
        context.dirty_offset = 0;

        Ok(Response::Ok)
    }
}
