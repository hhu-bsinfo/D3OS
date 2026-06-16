use terminal::println;

use crate::{
    built_in::built_in::BuiltIn,
    context::{context::ContextProvider, working_directory_context::WorkingDirectoryContext},
};

pub struct PwdBuiltIn {
    wd_provider: ContextProvider<WorkingDirectoryContext>,
}

impl BuiltIn for PwdBuiltIn {
    fn namespace(&self) -> &'static str {
        "pwd"
    }

    fn run(&mut self, args: &[&str]) -> usize {
        let wd_clx = self.wd_provider.borrow();

        if !args.is_empty() {
            Self::print_usage();
            return 1;
        }

        println!("{}", wd_clx.pwd());
        0
    }
}

impl PwdBuiltIn {
    pub fn new(wd_provider: ContextProvider<WorkingDirectoryContext>) -> Self {
        Self { wd_provider }
    }

    fn print_usage() {
        println!("Usage: pwd");
    }
}
