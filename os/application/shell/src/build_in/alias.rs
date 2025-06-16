use core::cell::RefCell;

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use terminal::{print, println};

use crate::sub_service::alias_sub_service::AliasSubService;

pub struct AliasBuildIn {
    args: Vec<String>,
    alias: Rc<RefCell<AliasSubService>>,
}

impl AliasBuildIn {
    pub fn new(args: Vec<&str>, alias: &Rc<RefCell<AliasSubService>>) -> Self {
        Self {
            args: args.into_iter().map(String::from).collect(),
            alias: alias.clone(),
        }
    }

    pub fn start(&self) -> Result<(), ()> {
        if self.args.is_empty() {
            return self.list();
        }

        if self.args.len() == 1 {
            return self.add();
        }

        return self.error();
    }

    fn list(&self) -> Result<(), ()> {
        let alias = self.alias.borrow();
        let entries = alias.get_all();
        if entries.is_empty() {
            println!("No entries");
            return Ok(());
        }

        for entry in entries {
            println!("{}={}", entry.key, entry.value);
        }
        Ok(())
    }

    fn add(&self) -> Result<(), ()> {
        let mut split = self.args.get(0).unwrap().splitn(2, "=");
        let key = match split.next() {
            Some(key) => key,
            None => return self.error(),
        };
        let value = match split.next() {
            Some(value) => value,
            None => return self.error(),
        };
        let stripped_value = match self.strip_quotes(value) {
            Ok(stripped_value) => stripped_value,
            Err(_) => return self.error(),
        };

        self.alias.borrow_mut().add(key, &stripped_value)
    }

    fn strip_quotes(&self, value: &str) -> Result<String, ()> {
        let bytes = value.as_bytes();
        if bytes.len() < 2 {
            return Err(());
        }

        let stripped_bytes = &value[1..value.len() - 1];

        if bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'' {
            return Ok(stripped_bytes.to_string());
        }
        if bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"' {
            return Ok(stripped_bytes.to_string());
        }

        Ok(value.to_string())
    }

    fn error(&self) -> Result<(), ()> {
        println!("Usage: alias KEY='VALUE'");
        Err(())
    }
}
