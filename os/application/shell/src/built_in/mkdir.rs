use naming::mkdir;
use naming::shared_types::{Capability, OpenOptions};
use terminal::println;

use crate::{
    built_in::built_in::BuiltIn,
    context::{context::ContextProvider, working_directory_context::WorkingDirectoryContext},
};

pub struct MkdirBuiltIn {
    wd_provider: ContextProvider<WorkingDirectoryContext>,
}

impl BuiltIn for MkdirBuiltIn {
    fn namespace(&self) -> &'static str {
        "mkdir"
    }

    fn run(&mut self, args: &[&str]) -> usize {
        let wd_clx = self.wd_provider.borrow();
        let Some(path) = args.get(0) else {
            Self::print_usage();
            return 1;
        };
        match mkdir(path, OpenOptions::CREATE, Capability::new(0)) {
            Ok(cap) => {
                println!("Debug: Successfully created directory '{}'", path);
                println!("Debug: Returned capability handle is {}", cap.handle());
            }
            Err(e) => {
                println!("Debug: Failed to create directory '{}', error: {:?}", path, e);
                Self::print_usage();
                return 1;
            }
        }
        0
    }
}

impl MkdirBuiltIn {
    pub fn new(wd_provider: ContextProvider<WorkingDirectoryContext>) -> Self {
        Self { wd_provider }
    }

    fn print_usage() {
        println!("Usage: mkdir DIRECTORY");
    }
}
