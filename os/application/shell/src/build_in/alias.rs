use core::cell::RefCell;

use alloc::{
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use terminal::{print, println};

use crate::sub_modules::alias::Alias;

pub struct AliasBuildIn {
    args: Vec<String>,
    alias: Rc<RefCell<Alias>>,
}

impl AliasBuildIn {
    pub fn new(args: Vec<&str>, alias: &Rc<RefCell<Alias>>) -> Self {
        Self {
            args: args.into_iter().map(String::from).collect(),
            alias: alias.clone(),
        }
    }

    pub fn start(&self) -> Result<(), ()> {
        if self.args.is_empty() {
            return self.list();
        }

        self.add()
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
        let raw = self.args.join(" ");
        let mut split = raw.splitn(2, "=");
        let key = split.next().unwrap_or("");
        let value = split.next().ok_or_else(|| self.usage().err().unwrap())?;

        let stripped_value = self.strip_quotes(value)?;
        self.alias.borrow_mut().set(key, &stripped_value);
        Ok(())
    }

    fn strip_quotes(&self, value: &str) -> Result<String, ()> {
        let bytes = value.as_bytes();

        if bytes.len() >= 2 {
            let first = bytes[0];
            let last = bytes[bytes.len() - 1];
            if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
                return Ok(value[1..value.len() - 1].to_string());
            }
        }

        Ok(value.to_string())
    }

    fn usage(&self) -> Result<(), ()> {
        println!("Usage: alias KEY=VALUE");
        Err(())
    }
}
