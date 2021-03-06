use crate::server::ServerMessage;
use crate::Input;
use actix::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use websocket::ClientBuilder;

#[derive(Message, Clone, Serialize, Deserialize)]
#[rtype(result = "()")]
pub enum ClientMessage {
    Sit {
        player_id: Option<i32>,
        name: String,
    },
    Sync,
    Input {
        input: Input,
        token: i32,
    },
}

pub struct Client {
    pub player_name: String,
    pub websocket_sender: websocket::sender::Writer<std::net::TcpStream>,
    pub message_queue: Arc<Mutex<VecDeque<String>>>,
}

impl Client {
    pub fn new(url: String, player_name: String) -> Result<Self, websocket::WebSocketError> {
        let msg_queue = Arc::new(Mutex::new(VecDeque::new()));
        let client = ClientBuilder::new(&url).unwrap().connect_insecure()?;

        let (mut receiver, sender) = client.split().unwrap();

        let msg_queue_clone = msg_queue.clone();

        std::thread::spawn(move || {
            for message in receiver.incoming_messages() {
                match message {
                    Ok(websocket::OwnedMessage::Text(text)) => {
                        msg_queue_clone.lock().unwrap().push_back(text);
                    }
                    Ok(_) => {}
                    Err(websocket::WebSocketError::NoDataAvailable) => {
                        eprintln!("[ERROR] Disconnected from server. Exiting...");
                        std::process::exit(1);
                    }
                    Err(err) => {
                        eprintln!("[ERROR] Unknown error occured: {:?}", err);
                        std::process::exit(1);
                    }
                }
            }
        });

        Ok(Client {
            player_name,
            websocket_sender: sender,
            message_queue: msg_queue,
        })
    }

    pub fn has_next(&self) -> bool {
        let queue = self.message_queue.lock().unwrap();
        !queue.is_empty()
    }

    pub fn next_message(&mut self) -> Option<ServerMessage> {
        let mut queue = self.message_queue.lock().unwrap();
        if queue.is_empty() {
            None
        } else {
            let serialized_message = queue.pop_front().unwrap();
            let message = serde_json::from_str::<ServerMessage>(&serialized_message)
                .expect("Failed to deserialize the server message");

            let truncated = {
                let mut formatted = format!("{:?}", message);
                if formatted.len() > 100 {
                    formatted.truncate(97);
                    formatted += " ...";
                }
                formatted
            };
            println!("Got a message: {}", truncated);
            Some(message)
        }
    }

    pub fn send(&mut self, message: &ClientMessage) {
        let serialized =
            serde_json::to_string(message).expect("Failed to serialize the client message.");

        self.websocket_sender
            .send_message(&websocket::Message::text(serialized))
            .expect("Failed to send the client message.")
    }

    pub fn send_sit(&mut self, player_id: Option<i32>) {
        self.send(&ClientMessage::Sit {
            player_id,
            name: self.player_name.clone(),
        });
    }

    pub fn send_input(&mut self, input: Input, token: i32) {
        self.send(&ClientMessage::Input { input, token });
    }

    pub fn send_sync(&mut self) {
        self.send(&ClientMessage::Sync);
    }
}
