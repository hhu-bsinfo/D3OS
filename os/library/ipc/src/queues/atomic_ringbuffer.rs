use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Mutex;

use crate::MAX_NUM_MESSAGES;

/// thread-safe, non-locking ringbuffer using atomics
pub struct Ringbuffer<T, const S: usize> {
    data: [T; S],
    head: AtomicUsize,
    tail: AtomicUsize,
    /// number of saved elements
    num_elements: AtomicUsize,
}

impl<T: Copy + Default, const S: usize> Ringbuffer<T, S> {

    pub fn new() -> Self {
        Self { data: [{ T::default() }; _], head: AtomicUsize::new(0), tail: AtomicUsize::new(0), num_elements: AtomicUsize::new(0) }
    }

    /// push `data` into the ringbuffer, if there is space
    pub fn push(&self, data: T) -> bool {

        // check if there is space -> increment num_elements here to avoid two threads fighting over a single free space
        let num_elements = self.num_elements.fetch_add(1, Ordering::SeqCst);

        if num_elements >= S {
            self.num_elements.fetch_sub(1, Ordering::SeqCst);
            return false;
        }

        // we now know there is at least one space free -> it needs to be at the tail

        // get and increment tail -> this effectively locks the entry pointed to by tail
        let tail_idx = self.tail.fetch_add(1, Ordering::SeqCst) % MAX_NUM_MESSAGES;

        // write the data
        unsafe { self.data.as_ptr().add(tail_idx).cast_mut().write(data); };
        true
    }

    /// check and reserve ringbuffer slot, for writing to later
    pub fn reserve(&self) -> Option<*mut T> {
        
        // check if there is space -> increment num_elements here to avoid two threads fighting over a single free space
        let num_elements = self.num_elements.fetch_add(1, Ordering::SeqCst);

        if num_elements >= S {
            self.num_elements.fetch_sub(1, Ordering::SeqCst);
            return None;
        }

        // we now know there is at least one space free -> it needs to be at the tail

        // get and increment tail -> this effectively locks the entry pointed to by tail
        let tail_idx = self.tail.fetch_add(1, Ordering::SeqCst) % MAX_NUM_MESSAGES;

        // return raw ptr to reserved slot
        Some(self.data.as_ptr().wrapping_add(tail_idx).cast_mut())
    }

    pub fn pop(&self) -> Option<T> {

        // check if there are any elements
        let num_elements = self.num_elements.load(Ordering::SeqCst);
        
        if num_elements == 0 {
            return None;
        }

        // get and increment head idx
        let head_idx = self.head.fetch_add(1, Ordering::SeqCst) % MAX_NUM_MESSAGES;

        // copy data from ringbuffer
        let data = unsafe { core::mem::take(self.data.as_ptr().add(head_idx).cast_mut().as_mut_unchecked()) };

        // there is now a free space
        self.num_elements.fetch_sub(1, Ordering::SeqCst);

        Some(data)
    }
    
    pub fn num_elements(&self) -> usize {
        self.num_elements.load(Ordering::Relaxed)
    }

    pub fn iter<'a>(&'a self) -> Iter<'a, T> {
        unimplemented!("this will probably not work if underlying data is modified while the iterator is being used -> maybe wrap each data element option in a mutex");
        Iter { data: &self.data, tail: self.tail.load(Ordering::Relaxed), idx: self.head.load(Ordering::Relaxed), size: S }
    }
}

struct Iter<'a, T> {
    data: &'a[T],
    tail: usize,
    idx: usize,
    size: usize,
}

impl<'a, T: 'a> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.idx >= self.tail {
            return None;
        }
        let item = &self.data[self.idx];
        self.idx = (self.idx + 1) & self.size;
        Some(item)
    }
}