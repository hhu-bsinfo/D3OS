use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use naming::cd;

use crate::event::event_handler::Error;

#[derive(Debug, Clone)]
pub struct WorkingDirectoryContext {
    components: Vec<String>,
}

impl WorkingDirectoryContext {
    pub fn new() -> Self {
        Self { components: Vec::new() }
    }

    pub fn pwd(&self) -> String {
        Self::components_to_string(&self.components)
    }

    pub fn resolve(&self, path: &str) -> String {
        let mut components = if path.starts_with('.') {
            self.components.clone()
        } else {
            Vec::new()
        };

        for part in path.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    components.pop();
                }
                segment => {
                    components.push(segment.to_string());
                }
            }
        }
        Self::components_to_string(&components)
    }

    pub fn cd(&mut self, path: &str) -> Result<(), Error> {
        let absolute_path = self.resolve(path);

        // Also navigate in kernel managed working directory to prevent conflicts
        // Can be replaced, with simple check if file / dir exists
        if cd(&absolute_path).is_err() {
            return Err(Error::new(
                format!("No such file or directory: {}", absolute_path),
                None,
            ));
        }

        let mut new_components = Vec::new();
        for part in absolute_path.split('/') {
            if part.is_empty() {
                continue;
            }
            new_components.push(part.to_string());
        }
        self.components = new_components;
        Ok(())
    }

    fn components_to_string(components: &[String]) -> String {
        if components.is_empty() {
            return "/".to_string();
        }

        let mut path = String::new();
        for component in components {
            path.push('/');
            path.push_str(component);
        }
        path
    }
}
