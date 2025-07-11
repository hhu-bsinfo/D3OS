pub trait BuiltIn {
    fn namespace(&self) -> &'static str;

    fn run(&mut self, args: &[&str]) -> isize;
}
