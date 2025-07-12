use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use crate::event::event_handler::Error;

#[derive(Debug, Default)]
pub struct AliasContext {
    entries: Vec<AliasEntry>,
}

#[derive(Debug)]
pub struct AliasEntry {
    pub(crate) key: String,
    pub(crate) value: String,
}

const INITIAL_ALIASES: &'static [(&'static str, &'static str)] = &[
    ("hhu", "Heinrich Heine Universitaet"),
    ("hi", "Hello there"),
    ("d3", "cargo make --no-workspace"),
    ("d3p", "cargo make --no-workspace --profile production"),
];
const MAX_ALIAS_LEN: usize = 16;

impl AliasContext {
    pub fn new() -> Self {
        let mut alias = Self::default();
        for (key, value) in INITIAL_ALIASES {
            alias.set(key, value);
        }
        alias
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<(), Error> {
        if let Some(pos) = self.find_position(key) {
            self.entries[pos].value = value.to_string();
            return Ok(());
        }

        if self.entries.len() >= MAX_ALIAS_LEN {
            return Err(Error::new("Too many aliases".to_string(), None));
        }

        self.entries.push(AliasEntry {
            key: key.to_string(),
            value: value.to_string(),
        });
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Result<(), ()> {
        let position = match self.find_position(key) {
            Some(position) => position,
            None => return Err(()),
        };

        self.entries.swap_remove(position);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        let position = match self.find_position(key) {
            Some(position) => position,
            None => return None,
        };

        match self.entries.get(position) {
            Some(entry) => Some(&entry.value),
            None => None,
        }
    }

    pub fn get_all(&self) -> &Vec<AliasEntry> {
        &self.entries
    }

    pub fn exist(&self, key: &str) -> bool {
        self.find_position(key).is_some()
    }

    fn find_position(&self, key: &str) -> Option<usize> {
        self.entries.iter().position(|entry| entry.key == key)
    }
}
