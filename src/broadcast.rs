use crate::server::BroadCast;
use std::collections::VecDeque;

pub struct BroadCastQueue {
    queue: VecDeque<BroadCast>,
}

impl BroadCastQueue {
    pub fn new() -> Self {
        BroadCastQueue {
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, broadcast: BroadCast) {
        self.queue.push_back(broadcast);
    }

    pub fn pop(&mut self) -> Option<BroadCast> {
        self.queue.pop_front()
    }
}

impl Iterator for BroadCastQueue {
    type Item = BroadCast;
    fn next(&mut self) -> Option<BroadCast> {
        self.pop()
    }
}
