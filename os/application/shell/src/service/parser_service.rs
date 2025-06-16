use alloc::string::ToString;
use logger::info;

use crate::{context::Context, executable::Executable, service::lexer_service::Token};

use super::service::{Error, Response, Service};

pub struct ParserService {}

impl Service for ParserService {
    fn prepare(&mut self, context: &mut Context) -> Result<Response, Error> {
        context.executable = None;
        Ok(Response::Ok)
    }

    fn submit(&mut self, context: &mut Context) -> Result<Response, Error> {
        self.parse(context)
    }
}

impl ParserService {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn parse(&mut self, context: &mut Context) -> Result<Response, Error> {
        let mut executable = Executable::new();

        context.tokens.iter().for_each(|token| match token {
            Token::Command(_clx, command) => {
                executable.create_job(command.to_string());
            }
            Token::Argument(_clx, argument) => {
                executable.add_argument_to_latest_job(argument.to_string());
            }
            _ => (),
        });

        info!("{:?}", &executable);
        context.executable = Some(executable);
        Ok(Response::Ok)
    }
}
