use crate::server::ArcServerMessage;
use std::collections::VecDeque;

pub struct BroadCastQueue {
    queue: VecDeque<ArcServerMessage>,
}

impl BroadCastQueue {
    pub fn new() -> Self {
        BroadCastQueue {
            queue: VecDeque::new(),
        }
    }

    pub fn push(&mut self, broadcast: ArcServerMessage) {
        self.queue.push_back(broadcast);
    }

    pub fn pop(&mut self) -> Option<ArcServerMessage> {
        self.queue.pop_front()
    }
}

impl Iterator for BroadCastQueue {
    type Item = ArcServerMessage;
    fn next(&mut self) -> Option<ArcServerMessage> {
        self.pop()
    }
}
