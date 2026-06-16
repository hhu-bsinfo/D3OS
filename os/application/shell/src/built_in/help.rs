use terminal::println;

use crate::built_in::built_in::BuiltIn;

pub struct HelpBuiltIn {}

impl BuiltIn for HelpBuiltIn {
    fn namespace(&self) -> &'static str {
        "help"
    }

    fn run(&mut self, args: &[&str]) -> usize {
        if args.is_empty() {
            println!("{}", include_str!("help.txt"));
            return 0;
        }
        if args.len() > 1 {
            Self::print_usage();
            return 1;
        }
        match *args.get(0).unwrap() {
            "tokens" => println!("{}", include_str!("help_tokens.txt")),
            "controls" => println!("{}", include_str!("help_controls.txt")),
            "built-in-1" => println!("{}", include_str!("help_built_in_1.txt")),
            "built-in-2" => println!("{}", include_str!("help_built_in_2.txt")),
            _ => {
                Self::print_usage();
                return 1;
            }
        }

        0
    }
}

impl HelpBuiltIn {
    pub fn new() -> Self {
        Self {}
    }

    fn print_usage() {
        println!("Usage: help [tokens / controls / built-in-1 / built-in-2]");
    }
}
