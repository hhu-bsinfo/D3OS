use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use logger::info;

#[derive(Debug)]
pub struct Alias {
    entries: Vec<AliasEntry>, // Todo#4 use lookup table instead??
}

#[derive(Debug)]
pub struct AliasEntry {
    pub(crate) key: String,
    pub(crate) value: String,
}

impl Alias {
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn add(&mut self, key: &str, value: &str) -> Result<(), ()> {
        if self.exist(key) {
            // TODO don't throw error, update value
            return Err(());
        }

        self.entries.push(AliasEntry {
            key: key.to_string(),
            value: value.to_string(),
        });

        info!("{:?}", self);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) -> Result<(), ()> {
        let position = match self.find_position(key) {
            Some(position) => position,
            None => return Err(()),
        };

        self.entries.swap_remove(position);
        info!("{:?}", self);
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
