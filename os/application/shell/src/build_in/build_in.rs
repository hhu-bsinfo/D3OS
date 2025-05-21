use alloc::vec::Vec;

// TODO simplify structure
pub trait BuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()>;
}
