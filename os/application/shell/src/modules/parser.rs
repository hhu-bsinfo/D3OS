use alloc::string::ToString;
use logger::info;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
    modules::lexer::token::Token,
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
        clx.tokens.get().iter().for_each(|token| match token {
            Token::Command(_clx, command) => {
                clx.executable.create_job(command.to_string());
            }
            Token::Argument(_clx, argument) => {
                clx.executable.add_argument_to_latest_job(argument.to_string());
            }
            _ => (),
        });

        info!("{:?}", &clx.executable);
        Ok(Response::Ok)
    }
}
