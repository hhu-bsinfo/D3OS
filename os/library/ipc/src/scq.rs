//! own implementation of the Scalable Circular Queue (SCQ) proposed in 
//! "A Scalable, Portable, and Memory-Efficient Lock-Free FIFO Queue" by Ruslan Nikolaev (arXiV/1908.04511)
//! done analog to lfqueue crate

use core::sync::atomic::{AtomicIsize, AtomicUsize, Ordering};

pub struct ScqRing<const N: usize> {
    head: AtomicUsize,
    tail: AtomicUsize,
    threshold: AtomicIsize,
    entries: [Ent; const {2*8}]
}

struct Ent {
    cycle: AtomicUsize,
    is_safe: AtomicUsize,
    index: AtomicUsize,
}

impl<const N: usize> ScqRing<N> {
    pub fn new() -> Self {
        Self {
            head: AtomicUsize::new(2 * N),
            tail: AtomicUsize::new(2 * N),
            threshold: AtomicIsize::new(-1),
            entries: [const { Ent { cycle: AtomicUsize::new(0), is_safe: AtomicUsize::new(1), index: AtomicUsize::new(0) } }; _],
        }
    }

    pub fn catchup(&self, tail: usize, head: usize) {
        while self.tail.compare_exchange(tail, head, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            let head = self.head.load(Ordering::SeqCst);
            let tail = self.tail.load(Ordering::SeqCst);
            if tail >= head {
                break;
            }
        }
    }

    pub fn enqueue(&self, index: usize) {
        loop {
            let T = self.tail.fetch_add(1, Ordering::SeqCst);
            //let j = cache_remap(T % 2*N);
            
        }
    }
}