use alloc::string::ToString;
use logger::info;
use terminal::DecodedKey;

use crate::{context::Context, executable::Executable, service::lexer_service::Token};

use super::service::{Service, ServiceError};

pub struct ParserService {}

impl Service for ParserService {
    fn run(&mut self, event: DecodedKey, context: &mut Context) -> Result<(), ServiceError> {
        match event {
            DecodedKey::Unicode('\n') => self.parse(context),
            _ => Ok(()),
        }
    }
}

impl ParserService {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn parse(&mut self, context: &mut Context) -> Result<(), ServiceError> {
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
        Ok(())
    }
}
