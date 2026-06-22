use alloc::{format, string::String};
use naming::shared_types::{FileType, OpenOptions};
use terminal::println;

use crate::{
    built_in::built_in::BuiltIn,
    context::{context::ContextProvider, working_directory_context::WorkingDirectoryContext},
};

pub struct LsBuiltIn {
    wd_provider: ContextProvider<WorkingDirectoryContext>,
}

impl BuiltIn for LsBuiltIn {
    fn namespace(&self) -> &'static str {
        "ls"
    }

    fn run(&mut self, args: &[&str]) -> usize {
        let wd_clx = self.wd_provider.borrow();
        let path = args.get(0).unwrap_or(&"");

        let absolute_path = wd_clx.resolve(path);
        let cap_handle = absolute_path.parse::<usize>().unwrap_or(0);
        let Ok(fd) = naming::open(naming::shared_types::Capability::new(cap_handle), OpenOptions::DIRECTORY) else {
            Self::print_usage();
            return 1;
        };

        let mut contents = String::new();
        while let Ok(Some(content)) = naming::readdir(fd) {
            contents.push_str(&format!(
                "{}{}\x1b[0m  ",
                Self::color_code_file_type(content.file_type),
                &content.name
            ));
        }

        println!("{}", contents);
        naming::close(fd).expect("Unable to close directory");

        0
    }
}

impl LsBuiltIn {
    pub fn new(wd_provider: ContextProvider<WorkingDirectoryContext>) -> Self {
        Self { wd_provider }
    }

    fn color_code_file_type(file_type: FileType) -> &'static str {
        match file_type {
            FileType::Directory => "",
            FileType::Link => "\x1b[38;2;128;128;128m",
            FileType::Regular => "\x1b[38;2;192;192;255m",
            _ => "",
        }
    }

    fn print_usage() {
        println!("Usage: ls [DIRECTORY]");
    }
}
