use naming::mkdir;
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

    fn run(&mut self, args: &[&str]) -> isize {
        let wd_clx = self.wd_provider.borrow();
        let Some(path) = args.get(0) else {
            Self::print_usage();
            return -1;
        };
        if mkdir(&wd_clx.resolve(path)).is_err() {
            Self::print_usage();
            return -1;
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
