use terminal::{DecodedKey, print, println};

use crate::{
    command_line::command_line::CommandLine,
    executor::executor::Executor,
    parser::{lexical_parser::LexicalParser, parser::Parser},
};

pub struct Controller {
    command_line: CommandLine,
    parser: LexicalParser,
    executor: Executor,
}

impl Controller {
    pub const fn new() -> Self {
        Self {
            command_line: CommandLine::new(),
            parser: LexicalParser::new(),
            executor: Executor::new(),
        }
    }

    fn handle_backspace(&mut self) {
        let _cursor_position = match self.command_line.remove_before_cursor() {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.parser.pop(); //TODO#1 THIS ONLY WORKS WHEN CURSOR IS AT LAST POS
    }

    fn handle_enter(&mut self) {
        self.command_line.submit();

        let executable = match self.parser.parse() {
            Ok(exec) => exec,
            Err(_) => return,
        };

        match self.executor.execute(&executable) {
            Ok(_) => return,
            Err(msg) => println!("{}", msg),
        }
    }

    fn handle_other_char(&mut self, ch: char) {
        let _cursor_position = match self.command_line.add_char(ch) {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.parser.push(ch); //TODO#1 THIS ONLY WORKS WHEN CURSOR IS AT LAST POS
    }

    pub fn run(&mut self, key: DecodedKey) {
        match key {
            DecodedKey::Unicode('\x08') => self.handle_backspace(),
            DecodedKey::Unicode('\n') => self.handle_enter(),
            DecodedKey::Unicode(ch) => self.handle_other_char(ch),
            _ => {}
        }
    }
}
