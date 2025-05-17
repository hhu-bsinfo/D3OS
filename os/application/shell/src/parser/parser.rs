use super::command_line::CommandLine;

pub trait Parser {
    fn push(&mut self, ch: char);
    fn pop(&mut self);
    fn parse(&mut self) -> CommandLine;
    fn reset(&mut self);
}
