use terminal::{DecodedKey, KeyCode, print, println};

use crate::{
    command_line::command_line::CommandLine, executor::executor::Executor, lexer::lexer::Lexer,
    parser::parser::Parser,
};

pub struct Controller {
    command_line: CommandLine,
    lexer: Lexer,
    parser: Parser,
    executor: Executor,
}

impl Controller {
    pub const fn new() -> Self {
        Self {
            command_line: CommandLine::new(),
            lexer: Lexer::new(),
            parser: Parser::new(),
            executor: Executor::new(),
        }
    }

    fn handle_backspace(&mut self) {
        let current_string = match self.command_line.remove_before_cursor() {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.lexer.tokenize(current_string); // TODO#? disable onChange updates when facing performance hits
    }

    fn handle_enter(&mut self) {
        self.command_line.submit();

        let tokens = self.lexer.get_tokens();
        let executable = match self.parser.parse(&tokens) {
            Ok(exec) => exec,
            Err(_) => return,
        };

        match self.executor.execute(&executable) {
            Ok(_) => return,
            Err(msg) => println!("{}", msg),
        }
    }

    fn handle_other_char(&mut self, ch: char) {
        let current_string = match self.command_line.add_char(ch) {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.lexer.tokenize(current_string); // TODO#? disable onChange updates when facing performance hits
    }

    fn handle_arrow_left(&mut self) {
        self.command_line.move_cursor_left();
    }

    fn handle_arrow_right(&mut self) {
        self.command_line.move_cursor_right();
    }

    pub fn run(&mut self, key: DecodedKey) {
        match key {
            // Unicodes
            DecodedKey::Unicode('\x08') => self.handle_backspace(),
            DecodedKey::Unicode('\n') => self.handle_enter(),
            DecodedKey::Unicode(ch) => self.handle_other_char(ch),
            // RawKeys
            DecodedKey::RawKey(KeyCode::ArrowLeft) => self.handle_arrow_left(),
            DecodedKey::RawKey(KeyCode::ArrowRight) => self.handle_arrow_right(),
            DecodedKey::RawKey(_) => {}
        }
    }
}
