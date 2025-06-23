use logger::info;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
    modules::lexer::token::TokenKind,
};

pub struct Parser {}

impl EventHandler for Parser {
    fn on_prepare_next_line(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.executable.reset();
        Ok(Response::Ok)
    }

    fn on_submit(&mut self, clx: &mut Context) -> Result<Response, Error> {
        self.parse(clx)
    }
}

impl Parser {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn parse(&mut self, clx: &mut Context) -> Result<Response, Error> {
        clx.tokens.get().iter().for_each(|token| match token.kind() {
            TokenKind::Command => {
                clx.executable.create_job(token.as_str());
            }
            TokenKind::Argument => {
                clx.executable.add_argument_to_latest_job(token.as_str());
            }
            _ => (),
        });

        info!("{:?}", &clx.executable);
        Ok(Response::Ok)
    }
}
