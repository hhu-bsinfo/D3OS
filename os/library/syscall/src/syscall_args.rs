pub trait SyscallArgs {
    fn arg(&self, index: usize) -> usize;
}

impl SyscallArgs for [usize; 0] {
    fn arg(&self, _index: usize) -> usize { 0 }
}

impl SyscallArgs for [usize; 1] {
    fn arg(&self, index: usize) -> usize { *self.get(index).unwrap_or(&0) }
}
impl SyscallArgs for [usize; 2] {
    fn arg(&self, index: usize) -> usize { *self.get(index).unwrap_or(&0) }
}
impl SyscallArgs for [usize; 3] {
    fn arg(&self, index: usize) -> usize { *self.get(index).unwrap_or(&0) }
}
impl SyscallArgs for [usize; 4] {
    fn arg(&self, index: usize) -> usize { *self.get(index).unwrap_or(&0) }
}
impl SyscallArgs for [usize; 5] {
    fn arg(&self, index: usize) -> usize { *self.get(index).unwrap_or(&0) }
}
impl SyscallArgs for [usize; 6] {
    fn arg(&self, index: usize) -> usize { *self.get(index).unwrap_or(&0) }
}