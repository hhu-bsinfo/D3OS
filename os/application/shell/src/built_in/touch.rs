use naming::touch;
use terminal::println;

use crate::{
    built_in::built_in::BuiltIn, context::{context::ContextProvider, working_directory_context::WorkingDirectoryContext}, event::event_handler::Response::Ok,
};

pub struct TouchBuiltIn {
    wd_provider: ContextProvider<WorkingDirectoryContext>,
}

impl BuiltIn for TouchBuiltIn {
    fn namespace(&self) -> &'static str {
        "touch"
    }

    fn run(&mut self, args: &[&str]) -> usize {
        let wd_clx = self.wd_provider.borrow();
        let Some(path) = args.get(0) else {
            Self::print_usage();
            return 1;
        };
        let res = touch(&wd_clx.resolve(path));
        match res {
          core::result::Result::Ok(_) => return 0,
          Err(e) => {
             println!("{:?}", e); 
             return 1;
          },
        };
    }
}

impl TouchBuiltIn {
    pub fn new(wd_provider: ContextProvider<WorkingDirectoryContext>) -> Self {
        Self { wd_provider }
    }

    fn print_usage() {
        println!("Usage: touch FILENAME");
    }
}
