use alloc::string::ToString;
use logger::info;

use crate::{
    context::context::Context,
    event::event_handler::{Error, EventHandler, Response},
    modules::lexer::Token,
};

pub struct Parser {}

impl EventHandler for Parser {
    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.executable.reset();
        Ok(Response::Ok)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.parse(context)
    }
}

impl Parser {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn parse(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.tokens.get().iter().for_each(|token| match token {
            Token::Command(_clx, command) => {
                context.executable.create_job(command.to_string());
            }
            Token::Argument(_clx, argument) => {
                context.executable.add_argument_to_latest_job(argument.to_string());
            }
            _ => (),
        });

        info!("{:?}", &context.executable);
        Ok(Response::Ok)
    }
}
