pub trait Parser {
    fn push(&mut self, ch: char);
    fn pop(&mut self);
    fn parse(&mut self); // -> CommandLine TODO#?
    fn reset(&mut self);
}
