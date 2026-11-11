/// Wrapper for Systemcall Arguments
/// 
/// allows passing 0 to 6 arguments normaly using an `usize slice`
/// but also accepts `raw ptr` and `ref` (for passing structs)

pub trait SyscallArgsElem {
    fn usize(&self) -> usize;
}

impl SyscallArgsElem for usize {
    fn usize(&self) -> usize {
        *self
    }
}

impl<T> SyscallArgsElem for *const T {
    fn usize(&self) -> usize {
        *self as usize
    }
}

impl<T> SyscallArgsElem for *mut T {
    fn usize(&self) -> usize {
        *self as usize
    }
}

impl<T> SyscallArgsElem for &T {
    fn usize(&self) -> usize {
        core::ptr::from_ref(*self) as usize
    }
}

impl<T> SyscallArgsElem for &mut T {
    fn usize(&self) -> usize {
        core::ptr::from_ref(*self) as usize
    }
}

pub trait SyscallArgs {
    fn arg(&self, index: usize) -> usize;
}

impl SyscallArgs for () {
    fn arg(&self, _index: usize) -> usize { 0 }
}

impl SyscallArgs for [usize; 0] {
    fn arg(&self, _index: usize) -> usize { 0 }
}

impl<T: SyscallArgsElem> SyscallArgs for T {
    fn arg(&self, index: usize) -> usize {
        if index == 0 { self.usize() } else { 0 }
    }
}

// by manually implementing arrays with 1 to 6 elements, the compiler will enforce the syscall argument limit for us

impl<T: SyscallArgsElem> SyscallArgs for [T; 1] {
    fn arg(&self, index: usize) -> usize { self.get(index).map(|elem| elem.usize()).unwrap_or(0) }
}
impl<T: SyscallArgsElem> SyscallArgs for [T; 2] {
    fn arg(&self, index: usize) -> usize { self.get(index).map(|elem| elem.usize()).unwrap_or(0) }
}
impl<T: SyscallArgsElem> SyscallArgs for [T; 3] {
    fn arg(&self, index: usize) -> usize { self.get(index).map(|elem| elem.usize()).unwrap_or(0) }
}
impl<T: SyscallArgsElem> SyscallArgs for [T; 4] {
    fn arg(&self, index: usize) -> usize { self.get(index).map(|elem| elem.usize()).unwrap_or(0) }
}
impl<T: SyscallArgsElem> SyscallArgs for [T; 5] {
    fn arg(&self, index: usize) -> usize { self.get(index).map(|elem| elem.usize()).unwrap_or(0) }
}
impl<T: SyscallArgsElem> SyscallArgs for [T; 6] {
    fn arg(&self, index: usize) -> usize { self.get(index).map(|elem| elem.usize()).unwrap_or(0) }
}