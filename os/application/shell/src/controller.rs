use core::cell::RefCell;

use alloc::rc::Rc;
use terminal::{DecodedKey, KeyCode, print, println};

use crate::{
    command_line::command_line::CommandLine, executor::executor::Executor, lexer::lexer::Lexer,
    parser::parser::Parser, sub_module::alias::Alias,
};

pub struct Controller {
    // Main Modules
    command_line: CommandLine,
    lexer: Lexer,
    parser: Parser,
    executor: Executor,
    // Sub Modules
    alias: Rc<RefCell<Alias>>,
}

impl Controller {
    pub fn new() -> Self {
        let alias = Rc::new(RefCell::new(Alias::new()));
        Self {
            command_line: CommandLine::new(),
            lexer: Lexer::new(),
            parser: Parser::new(),
            executor: Executor::new(alias.clone()),
            alias,
        }
    }

    fn handle_backspace(&mut self) {
        let current_string = match self.command_line.remove_before_cursor() {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.lexer.tokenize(&current_string); // TODO#? disable onChange updates when facing performance hits
    }

    fn handle_del(&mut self) {
        let current_string = match self.command_line.remove_at_cursor() {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.lexer.tokenize(&current_string); // TODO#? disable onChange updates when facing performance hits
    }

    fn handle_enter(&mut self) {
        self.command_line.submit();

        // Read tokens from lexer
        let tokens = match self.lexer.flush() {
            Ok(tokens) => tokens,
            Err(msg) => return self.handle_error(msg),
        };

        // Parse tokens into executables
        let executable = match self.parser.parse(&tokens) {
            Ok(exec) => exec,
            Err(_) => return,
        };

        // Execute
        match self.executor.execute(&executable) {
            Ok(_) => self.command_line.create_new_line(),
            Err(msg) => self.handle_error(msg),
        };
    }

    fn handle_other_char(&mut self, ch: char) {
        let current_string = match self.command_line.add_char(ch) {
            Ok(pos) => pos,
            Err(_) => return,
        };

        self.lexer.tokenize(&current_string); // TODO#? disable onChange updates when facing performance hits
    }

    fn handle_arrow_left(&mut self) {
        self.command_line.move_cursor_left();
    }

    fn handle_arrow_right(&mut self) {
        self.command_line.move_cursor_right();
    }

    fn handle_arrow_up(&mut self) {
        match self.command_line.move_history_up() {
            Ok(line) => self.lexer.tokenize(&line),
            Err(_) => return,
        };
    }

    fn handle_arrow_down(&mut self) {
        match self.command_line.move_history_down() {
            Ok(line) => self.lexer.tokenize(&line),
            Err(_) => return,
        };
    }

    fn handle_error(&mut self, msg: &'static str) {
        println!("{}", msg);
        self.command_line.create_new_line();
    }

    pub fn init(&mut self) {
        self.command_line.create_new_line();
    }

    pub fn run(&mut self, key: DecodedKey) {
        match key {
            // Unicodes
            DecodedKey::Unicode('\x08') => self.handle_backspace(),
            DecodedKey::Unicode('\x7F') => self.handle_del(),
            DecodedKey::Unicode('\n') => self.handle_enter(),
            DecodedKey::Unicode(ch) => self.handle_other_char(ch),
            // RawKeys
            DecodedKey::RawKey(KeyCode::ArrowLeft) => self.handle_arrow_left(),
            DecodedKey::RawKey(KeyCode::ArrowRight) => self.handle_arrow_right(),
            DecodedKey::RawKey(KeyCode::ArrowUp) => self.handle_arrow_up(),
            DecodedKey::RawKey(KeyCode::ArrowDown) => self.handle_arrow_down(),
            DecodedKey::RawKey(_) => {}
        }
    }
}
