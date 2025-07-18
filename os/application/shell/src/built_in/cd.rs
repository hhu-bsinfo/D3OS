use terminal::println;

use crate::{
    built_in::built_in::BuiltIn,
    context::{context::ContextProvider, working_directory_context::WorkingDirectoryContext},
};

pub struct CdBuiltIn {
    wd_provider: ContextProvider<WorkingDirectoryContext>,
}

impl BuiltIn for CdBuiltIn {
    fn namespace(&self) -> &'static str {
        "cd"
    }

    fn run(&mut self, args: &[&str]) -> isize {
        let mut wd_clx = self.wd_provider.borrow_mut();
        let Some(dir) = args.get(0) else {
            Self::print_usage();
            return -1;
        };
        if let Err(error) = wd_clx.cd(dir) {
            println!("{}", error.message);
            return -1;
        }

        0
    }
}

impl CdBuiltIn {
    pub fn new(wd_provider: ContextProvider<WorkingDirectoryContext>) -> Self {
        Self { wd_provider }
    }

    fn print_usage() {
        println!("Usage: cd DIRECTORY");
    }
}
