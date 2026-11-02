
use spin::rwlock::RwLock;

use crate::queues::{YieldD3OS, atomic_ringbuffer::Ringbuffer};

/// thread-safe, rarely locking, fixed sized queue using atomic_ringbuffer
pub struct Ringqueue<T, const S: usize> {
    /// data stores in RwLock to allow mutable access, and ensure atomic read/write operations
    data: [RwLock<T, YieldD3OS>; S],
    free_slots: Ringbuffer<usize, S>,
    used_slots: Ringbuffer<usize, S>,
}

impl<T: Copy + Default, const S: usize> Ringqueue<T, S> {

    pub fn new() -> Self {
        let s = Self { data: core::array::from_fn(|_| RwLock::new(T::default())), free_slots: Ringbuffer::new(), used_slots: Ringbuffer::new() };
        // init free slots
        for i in 0..S {
            s.free_slots.push(i);
        }
        s
    }

    /// push `data` into the queue, if there is space
    pub fn push(&self, data: T) -> bool {

        // check if there is a free slot available
        if let Some(free_slot) = self.free_slots.pop() {

            // write the data
            // aquire write lock to ensure atomic writing operation
            // this should always succeed
            if let Some(_guard) = self.data[free_slot].try_write() {
                let ptr = self.data[free_slot].as_mut_ptr();
                unsafe { ptr.write(data); }; // copy data here
                self.used_slots.push(free_slot); // mark slot as used
                return true;
            }
        }

        false
    }

    /// check and reserve ringbuffer slot, for writing to later
    pub fn reserve(&self) -> Option<(*mut T, spin::rwlock::RwLockWriteGuard<'_, T, YieldD3OS>)> {

        // check if there is a free slot available
        if let Some(free_slot) = self.free_slots.pop() {

            // return raw ptr to reserved slot -> try_write should always succeed
            if let Some(guard) = self.data[free_slot].try_write() {
                self.used_slots.push(free_slot); // mark slot as used
                Some((self.data[free_slot].as_mut_ptr(), guard))
            } else {
                // this slot isnt actually free, do not re-queue?
                //self.free_slots.push(free_slot); // re-queue the free slot
                None
            }
        } else {
            None
        }
    }

    /// take the next `T` (if there is one), copying the data
    pub fn pop(&self) -> Option<T> {

        // check if there is a used slot (the queue contains data)
        if let Some(used_slot) = self.used_slots.pop() {
            
            // return a ref to the data in the used slot
            if let Some(_guard) = self.data[used_slot].try_read() {
                let ptr = self.data[used_slot].as_mut_ptr();
                return Some(unsafe { ptr.read() }); // copy here
            }
        }

        None
    }

    /// take a ref to the next `T` (if there is one), does not copy
    pub fn pop_ref(&self) -> Option<RingqueueItemWrapper<'_, T, S>> {

        // check if there is a used slot (the queue contains data)
        if let Some(used_slot) = self.used_slots.pop() {
            
            // return a ref to the data in the used slot
            // try read here just in case
            if let Some(_guard) = self.data[used_slot].try_read() {
                return Some(RingqueueItemWrapper { ringqueue_ref: self, slot: used_slot });
            }
        }

        None
    }

    /// return a ref to the data in the given slot \
    /// unsafe unless the slot was previously popped using the corresponding function
    fn read(&self, slot: usize) -> Option<&T> {
        if let Some(_guard) = self.data[slot].try_read() {
            let ptr = self.data[slot].as_mut_ptr();
            unsafe { ptr.as_ref() }
        } else {
            None
        }
    }

    /// frees the value at the given slot
    fn free(&self, slot: usize) {
        // busy wait here; should never happen, we should always be the sole slot owner here, but just in case
        let _guard = self.data[slot].write();
        // busy wait here; this should never happen, there should never be more slots then this ringbuffers size
        while !self.free_slots.push(slot) {
            concurrent::thread::switch();
        }
    }
    
    pub fn num_free(&self) -> usize {
        self.free_slots.num_elements()
    }
    
    pub fn num_used(&self) -> usize {
        self.used_slots.num_elements()
    }
}

pub struct RingqueueItemWrapper<'a, T: Copy + Default, const S: usize> {
    ringqueue_ref: &'a Ringqueue<T, S>,
    slot: usize
}

impl<'a, T: Copy + Default, const S: usize> RingqueueItemWrapper<'a, T, S> {
    pub fn read(&self) -> Option<&T> {
        self.ringqueue_ref.read(self.slot)
    }
}

impl<'a, T: Copy + Default, const S: usize> Drop for RingqueueItemWrapper<'a, T, S> {
    fn drop(&mut self) {
        self.ringqueue_ref.free(self.slot);
    }
}