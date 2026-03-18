/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: irqsave_spinlock                                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ A irq save and multicore save spinlock for a generic data type.         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 01.09.2025               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use crate::device::cpu; 
use core::sync::atomic::{AtomicBool, Ordering};
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::cell::Cell;
use core::ops::{Deref, DerefMut};


/// An IRQ-save spinlock protecting a value of type `T`.
pub struct IrqSaveSpinlock<T> {
    locked: AtomicBool,        // false = unlocked, true = locked
    value: UnsafeCell<T>,
    _pd: PhantomData<*const T>, // !Send/Sync auto inference; we implement manually below
}

// Safety: we enforce exclusive &mut access via the lock,
// so Send/Sync depend on T like other standard locks:
unsafe impl<T: Send> Send for IrqSaveSpinlock<T> {}
unsafe impl<T: Send> Sync for IrqSaveSpinlock<T> {}

pub struct IrqSaveGuard<'a, T> {
    lock: &'a IrqSaveSpinlock<T>,
    irq_prev: Cell<bool>,
}

impl<T> IrqSaveSpinlock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: UnsafeCell::new(value),
            _pd: PhantomData,
        }
    }


    /// Acquire the lock, disabling local IRQs until the guard is dropped.
    #[inline]
    pub fn lock(&self) -> IrqSaveGuard<'_, T> {
        let prev = cpu::disable_int_nested();
    
        // spin until we transition false -> true
        while self.locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            cpu::pause();
        }

        IrqSaveGuard { lock: self, irq_prev: Cell::new(prev) }
    }

}

impl<'a, T> Drop for IrqSaveGuard<'a, T> {
    #[inline]
    fn drop(&mut self) {
        // Release lock first to publish writes, then restore CPU state.
        self.lock.locked.store(false, Ordering::Release);
        let prev = self.irq_prev.get();
        cpu::enable_int_nested(prev);
    }
}

impl<'a, T> Deref for IrqSaveGuard<'a, T> {
    type Target = T;
    #[inline] fn deref(&self) -> &T { unsafe { &*self.lock.value.get() } }
}
impl<'a, T> DerefMut for IrqSaveGuard<'a, T> {
    #[inline] fn deref_mut(&mut self) -> &mut T { unsafe { &mut *self.lock.value.get() } }
}
