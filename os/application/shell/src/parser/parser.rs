use super::executable::Executable;

pub trait Parser {
    fn push(&mut self, ch: char);
    fn pop(&mut self);
    fn parse(&mut self) -> Result<Executable, ()>;
    fn reset(&mut self);
}
