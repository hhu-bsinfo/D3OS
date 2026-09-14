

struct Entry<T> {
    object: Option<T>,
    next: Option<usize>,
    prev: Option<usize>,
}

/// fixed sized queue using an internal buffer
pub struct BufferQueue<T, const S: usize> {
    buffer: [Entry<T>; S],
    front: usize,
    middle: Option<usize>,
    back: usize,
    num_objects: usize,
}

impl<T, const S: usize> BufferQueue<T, S> {

    pub fn new() -> Self {
        Self { buffer: core::array::from_fn(|i| {
            let next = if i != S-1 { Some(i + 1) } else { None };
            let prev = if i != 0 { Some(i - 1) } else { None };
            Entry { object: None, next, prev }
        }), front: 0, middle: None, back: S-1, num_objects: 0 }
    }

    pub fn enqueue(&mut self, object: T) -> bool {
        match self.middle {
            // there exists an entry
            Some(middle_idx) => {
                let middle_entry = &self.buffer[middle_idx];
                match middle_entry.next {
                    Some(next_idx) => {
                        self.get(next_idx).object = Some(object);
                        self.middle = Some(next_idx);
                        self.num_objects += 1;
                        true
                    },
                    None => false,
                }
            },
            // this is the first enqueue, queue is curently empty
            None => {
                self.get(0).object = Some(object);
                self.middle = Some(0);
                self.num_objects += 1;
                true
            },
        }
    }

    pub fn dequeue(&mut self, idx: usize) -> Option<T> {

        if idx >= S { return None; }

        if let Some(middle_idx) = self.middle {
            if middle_idx == idx {
                // this is always None?
                self.middle = self.get(idx).next;
            }
        }

        self.num_objects -= 1;

        if idx == self.back { return core::mem::take(&mut self.get(idx).object); }

        if let Some(prev_idx) = self.get(idx).prev {
            self.get(prev_idx).next = self.get(idx).next;
        }

        self.get_back().next = Some(idx);
        self.get(idx).prev = Some(self.back);
        self.get(idx).next = None;

        self.back = idx;

        // take value out of queue
        core::mem::take(&mut self.get(idx).object)
    }

    pub fn dequeue_front(&mut self) -> Option<T> {
        self.dequeue(self.front)
    }

    pub fn peek(&self, idx: usize) -> Option<&T> {

        if idx >= S { return None; }

        (&self.buffer[idx].object).as_ref()
    }

    pub fn peek_front(&self) -> Option<&T> {
        self.peek(self.front)
    }

    fn get(&mut self, idx: usize) -> &mut Entry<T> {
        let i = idx % S;
        &mut self.buffer[i]
    }
    fn get_front(&mut self) -> &mut Entry<T> {
        self.get(self.front)
    }
    fn get_back(&mut self) -> &mut Entry<T> {
        self.get(self.back)
    }
    fn get_middle(&mut self) -> Option<&mut Entry<T>> {
        Some(self.get(self.middle?))
    }

    pub fn front(&self) -> usize {
        self.front
    }
    pub fn back(&self) -> usize {
        self.back
    }
    pub fn num_objects(&self) -> usize {
        self.num_objects
    }
}