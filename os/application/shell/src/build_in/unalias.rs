use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};
use terminal::{print, println};

use crate::context::alias_context::AliasContext;

pub struct UnaliasBuildIn {
    args: Vec<String>,
    alias_provider: Rc<RefCell<AliasContext>>,
}

impl UnaliasBuildIn {
    pub fn new(args: Vec<&str>, alias_provider: Rc<RefCell<AliasContext>>) -> Self {
        Self {
            args: args.into_iter().map(String::from).collect(),
            alias_provider,
        }
    }

    pub fn start(&self) -> Result<(), ()> {
        if self.args.len() != 1 {
            return self.error();
        }

        let key = self.args.get(0).unwrap();
        match self.alias_provider.borrow_mut().remove(key) {
            Ok(_) => println!("Removed {}", key),
            Err(_) => println!("Alias not found"),
        };

        Ok(())
    }

    fn error(&self) -> Result<(), ()> {
        println!("Usage: unalias KEY");
        Err(())
    }
}
