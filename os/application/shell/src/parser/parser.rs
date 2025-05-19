use alloc::{string::ToString, vec::Vec};
use logger::info;

use crate::lexer::lexer::Token;

use super::executable::Executable;

pub struct Parser {}

impl Parser {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn parse(&mut self, tokens: &Vec<Token>) -> Result<Executable, ()> {
        let mut executable = Executable::new();

        tokens.iter().for_each(|token| match token {
            Token::Command(command) => {
                executable.create_job(command.to_string());
            }
            Token::Argument(argument) => {
                executable.add_argument_to_latest_job(argument.to_string());
            }
        });

        info!("{:?}", &executable);
        Ok(executable)
    }
}
