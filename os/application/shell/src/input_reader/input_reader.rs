use core::cell::RefCell;

use alloc::rc::Rc;
use terminal::{DecodedKey, print, read::read_mixed};

use crate::{module::Module, state::State};

pub struct InputReader {
    state: Rc<RefCell<State>>,
    position: usize,
}

impl InputReader {
    pub const fn new(state: Rc<RefCell<State>>) -> Self {
        Self { state, position: 0 }
    }

    fn handle_backspace(&mut self) {
        if self.position == 0 {
            return;
        }

        print!("\x1b[1D \x1b[1D");

        let mut state = self.state.borrow_mut();
        state.current_line.pop();
        self.position -= 1;
    }

    fn handle_enter(&mut self) {
        print!("\n");

        let mut state = self.state.borrow_mut();
        state.submit = true;
    }

    fn handle_other_chars(&mut self, ch: char) {
        print!("{}", ch);

        let mut state = self.state.borrow_mut();
        state.current_line.push(ch);
        state.read_char = Some(ch);
        self.position += 1;
    }
}

impl Module for InputReader {
    fn run(&mut self) {
        let key = match read_mixed() {
            Some(key) => key,
            None => return,
        };

        // TODO support del key
        match key {
            DecodedKey::Unicode('\x08') => self.handle_backspace(),
            DecodedKey::Unicode('\n') => self.handle_enter(),
            DecodedKey::Unicode(ch) => self.handle_other_chars(ch),
            // TODO handle raw keys
            _ => {}
        };
    }
}
