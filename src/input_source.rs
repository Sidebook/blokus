use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
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
    Undo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserInput {
    pub player_id: i32,
    pub token: Option<i32>,
    pub input: Input,
}

pub struct InputQueue {
    pub queue: VecDeque<UserInput>,
    pub broadcast_requested: bool,
}

impl InputQueue {
    pub fn new() -> Self {
        InputQueue {
            queue: VecDeque::new(),
            broadcast_requested: false,
        }
    }
    pub fn push(&mut self, user_input: UserInput) {
        self.queue.push_back(user_input);
    }

    pub fn pop(&mut self) -> Option<UserInput> {
        self.queue.pop_front()
    }
}
