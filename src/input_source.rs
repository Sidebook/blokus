use rltk::console;
use std::sync::Arc;
use std::collections::VecDeque;
use std::sync::Mutex;

pub enum Input {
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
    // pub sources: Vec<&'a dyn InputSource>,
}

impl InputQueue {

    pub fn new() -> Self {
        InputQueue{
            queue: VecDeque::new(),
        }
    }
    pub fn push(&mut self, player_id: i32, input: Input) {
        self.queue.push_back((player_id, input));
    }

    pub fn pop(&mut self) -> Option<(i32, Input)> {
        self.queue.pop_front()
    }

    pub fn pop_for(&mut self, player_id: i32) -> Option<Input> {
        while !self.queue.is_empty() {
            let (pid, i) = self.pop().unwrap();
            if pid == player_id {
                return Some(i);
            }
        }
        None
    }
}
