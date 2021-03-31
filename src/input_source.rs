use std::collections::VecDeque;

pub enum Input {
    RequestBroadcast,
    Left,
    Right,
    Up,
    Down,
    RotateRight,
    RotateLeft,
    Flip,
    GiveUp,
    Cancel,
    Enter,
}

pub struct InputQueue {
    pub queue: VecDeque<(i32, Input)>,
    pub broadcast_requested: bool,
    // pub sources: Vec<&'a dyn InputSource>,
}

impl InputQueue {
    pub fn new() -> Self {
        InputQueue {
            queue: VecDeque::new(),
            broadcast_requested: false
        }
    }
    pub fn push(&mut self, player_id: i32, input: Input) {
        self.queue.push_back((player_id, input));
    }

    pub fn pop(&mut self) -> Option<(i32, Input)> {
        match self.queue.pop_front() {
            Some((_, Input::RequestBroadcast)) => {
                self.broadcast_requested = true;
                self.pop()
            }
            otherwise => otherwise
        }
    }

    pub fn consume_broadcast(&mut self) -> bool {
        let broadcast_requested = self.broadcast_requested;
        self.broadcast_requested = false;
        broadcast_requested
    }

    pub fn pop_for(&mut self, player_id: i32) -> Option<Input> {
        while !self.queue.is_empty() {
            if let Some((pid, i)) = self.pop() {
                if pid == player_id {
                    return Some(i);
                }
            }
        }
        None
    }
}
