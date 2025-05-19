use alloc::vec::Vec;

pub trait BuildIn {
    fn start(args: Vec<&str>) -> Result<(), ()>;
}
