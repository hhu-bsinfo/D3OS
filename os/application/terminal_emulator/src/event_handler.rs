use alloc::collections::vec_deque::VecDeque;

pub enum Event {
    EnterGuiMode,
}

pub struct EventHandler {
    queue: VecDeque<Event>,
}

impl EventHandler {
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    pub fn trigger(&mut self, event: Event) {
        self.queue.push_back(event);
    }

    pub fn handle(&mut self) -> Option<Event> {
        self.queue.pop_front()
    }
}
