use alloc::collections::vec_deque::VecDeque;

use crate::event::event::Event;

#[derive(Debug, Clone)]
pub struct EventBus {
    events: VecDeque<Event>,
}

impl EventBus {
    pub const fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    pub fn trigger(&mut self, event: Event) {
        self.events.push_back(event);
    }

    pub fn process(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }
}
