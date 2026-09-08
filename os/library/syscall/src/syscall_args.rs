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

impl SyscallArgs for usize {
    fn arg(&self, index: usize) -> usize {
        if index == 0 { *self } else { 0 }
    }
}

impl<T: Sized> SyscallArgs for *const T {
    fn arg(&self, index: usize) -> usize {
        if index == 0 { *self as usize } else { 0 }
    }
}

impl<T: Sized> SyscallArgs for &T {
    fn arg(&self, index: usize) -> usize {
        if index == 0 { core::ptr::from_ref(*self) as usize } else { 0 }
    }
}