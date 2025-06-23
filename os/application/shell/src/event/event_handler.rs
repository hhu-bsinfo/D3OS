use terminal::DecodedKey;

use crate::context::context::Context;

#[derive(Debug, PartialEq)]
pub enum Response {
    Ok,
    Skip,
    Ignore,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub(crate) message: &'static str,
    pub(crate) reason: Option<&'static str>,
    pub(crate) hint: Option<&'static str>,
}

impl Error {
    pub const fn new(message: &'static str, reason: Option<&'static str>, hint: Option<&'static str>) -> Self {
        Self { message, reason, hint }
    }
}

#[allow(unused_variables)]
pub trait EventHandler {
    fn on_key_pressed(&mut self, clx: &mut Context, key: DecodedKey) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_cursor_moved(&mut self, clx: &mut Context, step: isize) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_history_restored(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_line_written(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_tokens_written(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_process_completed(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }
}
