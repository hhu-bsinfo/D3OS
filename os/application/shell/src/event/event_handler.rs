use crate::context::context::Context;

#[derive(Debug)]
pub enum Response {
    Ok,
    Skip,
    Ignore,
}

#[derive(Debug)]
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
    fn prepare(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn history_up(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn history_down(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn cursor_left(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn cursor_right(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn auto_complete(&mut self, clx: &mut Context) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }

    fn simple_key(&mut self, clx: &mut Context, key: char) -> Result<Response, Error> {
        Ok(Response::Ignore)
    }
}
