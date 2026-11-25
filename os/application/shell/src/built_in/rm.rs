use naming::unlink;
use syscall::return_vals::Errno;
use terminal::println;

use crate::{
    built_in::built_in::BuiltIn,
    context::{context::ContextProvider, working_directory_context::WorkingDirectoryContext},
};

pub struct RmBuiltIn {
    wd_provider: ContextProvider<WorkingDirectoryContext>,
}

impl BuiltIn for RmBuiltIn {
    fn namespace(&self) -> &'static str {
        "rm"
    }

    /// Remove each FILE given in `args`. Keeps going if one fails and
    /// returns 1 if at least one file could not be removed.
    fn run(&mut self, args: &[&str]) -> usize {
        if args.is_empty() {
            Self::print_usage();
            return 1;
        }

        let wd_clx = self.wd_provider.borrow();
        let mut status = 0;
        for path in args {
            if let Err(e) = unlink(&wd_clx.resolve(path)) {
                println!("rm: cannot remove '{}': {}", path, Self::describe(e));
                status = 1;
            }
        }
        status
    }
}

impl RmBuiltIn {
    pub fn new(wd_provider: ContextProvider<WorkingDirectoryContext>) -> Self {
        Self { wd_provider }
    }

    fn print_usage() {
        println!("Usage: rm FILE...");
    }

    fn describe(e: Errno) -> &'static str {
        match e {
            Errno::ENOENT => "No such file or directory",
            Errno::EISDIR => "Is a directory",
            Errno::ENOTDIR => "Not a directory",
            Errno::ENOTSUP => "Operation not supported",
            Errno::EINVAL => "Invalid argument",
            Errno::ERDONLY => "Read-only file system",
            _ => "Unknown error",
        }
    }
}
