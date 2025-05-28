use core::cell::RefCell;

use alloc::{rc::Rc, string::String, vec::Vec};
use terminal::{print, println};

use crate::sub_service::alias_sub_service::AliasSubService;

pub struct UnaliasBuildIn {
    args: Vec<String>,
    alias: Rc<RefCell<AliasSubService>>,
}

impl UnaliasBuildIn {
    pub fn new(args: Vec<&str>, alias: &Rc<RefCell<AliasSubService>>) -> Self {
        Self {
            args: args.into_iter().map(String::from).collect(),
            alias: alias.clone(),
        }
    }

    pub fn start(&self) -> Result<(), ()> {
        if self.args.len() != 1 {
            return self.error();
        }

        let key = self.args.get(0).unwrap();
        match self.alias.borrow_mut().remove(key) {
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
