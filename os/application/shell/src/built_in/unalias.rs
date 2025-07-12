use core::cell::RefCell;

use alloc::rc::Rc;
use terminal::{print, println};

use crate::{built_in::built_in::BuiltIn, context::alias_context::AliasContext};

pub struct UnaliasBuiltIn {
    alias_provider: Rc<RefCell<AliasContext>>,
}

impl BuiltIn for UnaliasBuiltIn {
    fn namespace(&self) -> &'static str {
        "unalias"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        if args.len() != 1 {
            Self::print_usage();
            return -1;
        }

        let key = args.get(0).unwrap();
        match self.alias_provider.borrow_mut().remove(key) {
            Ok(_) => {
                println!("Removed {}", key);
                0
            }
            Err(_) => {
                println!("Alias not found. Did you wrap the key in quotes?");
                Self::print_usage();
                -1
            }
        }
    }
}

impl UnaliasBuiltIn {
    pub fn new(alias_provider: Rc<RefCell<AliasContext>>) -> Self {
        Self { alias_provider }
    }

    fn print_usage() {
        println!("Usage: unalias 'KEY'");
    }
}
