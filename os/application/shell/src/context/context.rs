use core::cell::{Ref, RefCell, RefMut};

use alloc::rc::Rc;

#[derive(Debug, Clone)]
pub struct ContextProvider<T> {
    clx: Rc<RefCell<T>>,
}

impl<T> ContextProvider<T> {
    pub fn new(clx: T) -> Self {
        Self {
            clx: Rc::new(RefCell::new(clx)),
        }
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        self.clx.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.clx.borrow_mut()
    }
}
