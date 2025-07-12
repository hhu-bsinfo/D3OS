use core::cell::RefCell;

use alloc::{
    rc::Rc,
    string::{String, ToString},
};
use terminal::{print, println};

use crate::{built_in::built_in::BuiltIn, context::alias_context::AliasContext};

pub struct AliasBuiltIn {
    alias_provider: Rc<RefCell<AliasContext>>,
}

impl BuiltIn for AliasBuiltIn {
    fn namespace(&self) -> &'static str {
        "alias"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        if args.is_empty() {
            return self.list_aliases();
        }

        self.set_alias(args)
    }
}

impl AliasBuiltIn {
    pub fn new(alias_provider: Rc<RefCell<AliasContext>>) -> Self {
        Self { alias_provider }
    }

    fn list_aliases(&self) -> isize {
        let alias_clx = self.alias_provider.borrow();
        let entries = alias_clx.get_all();
        if entries.is_empty() {
            println!("No entries");
            return 0;
        }

        for entry in entries {
            println!("{}={}", entry.key, entry.value);
        }
        0
    }

    fn set_alias(&self, args: &[&str]) -> isize {
        let raw = args.join(" ");
        let mut split = raw.splitn(2, "=");
        let key = split.next().unwrap_or("");
        let Ok(value) = split.next().ok_or_else(|| Self::print_usage()) else {
            Self::print_usage();
            return -1;
        };
        let Ok(stripped_value) = Self::strip_quotes(value) else {
            Self::print_usage();
            return -1;
        };

        let mut alias_clx = self.alias_provider.borrow_mut();
        if let Err(error) = alias_clx.set(key, &stripped_value) {
            println!("{}", error.message);
            return -1;
        };

        0
    }

    fn strip_quotes(value: &str) -> Result<String, ()> {
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

    fn print_usage() {
        println!("Usage: alias KEY=VALUE");
    }
}
