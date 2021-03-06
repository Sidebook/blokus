use std::collections::HashSet;
use super::{Input, InputQueue};
use crate::ClientMessage;
use crate::UserInput;
use actix::prelude::*;
use actix_web::web::Data;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::Arc;
use std::sync::Mutex;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum ServerMessage {
    Sync {
        serialized_data: String,
        trigger: UserInput,
    },
    Accept {
        player_id: i32,
    },
    Reject {
        reason: String,
    },
}

#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct ArcServerMessage {
    pub message: Arc<ServerMessage>,
}

#[derive(Clone)]
pub struct PlayerSlot {
    pub id: usize,
    pub name: String,
}

pub struct PlayerSlotManager {
    slots: Vec<Option<PlayerSlot>>,
    updated: bool,
}

#[derive(Debug)]
pub enum SlotRequestError {
    IndexOutOfRange,
    AlreadyExists,
}

impl PlayerSlotManager {
    pub fn new(n: usize) -> Self {
        PlayerSlotManager {
            slots: vec![None; n],
            updated: false,
        }
    }

    pub fn request(&mut self, slot: PlayerSlot) -> Result<(), SlotRequestError> {
        let id = slot.id;
        if self.slots.len() <= id {
            return Err(SlotRequestError::IndexOutOfRange);
        }
        if let Some(_) = self.slots[id] {
            return Err(SlotRequestError::AlreadyExists);
        }

        self.slots[id] = Some(slot);
        self.updated = true;
        Ok(())
    }

    pub fn get<'a>(&'a mut self, id: usize) -> Option<&'a PlayerSlot> {
        self.slots[id].as_ref()
    }

    pub fn remove(&mut self, id: usize) {
        self.slots[id] = None;
        self.updated = true;
    }

    pub fn consume_updated(&mut self) -> bool {
        let updated = self.updated;
        self.updated = false;
        updated
    }

    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn empty_slots(&self) -> Vec<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, e)| e.is_none())
            .map(|(i, _)| i)
            .collect()
    }
}

struct WebSocketSession {
    ism: Data<Mutex<InputQueue>>,
    slot_manager: Data<Mutex<PlayerSlotManager>>,
    ws_monitor: Data<Addr<WebsocketSessionMonitor>>,
    slot: Option<PlayerSlot>,
}

impl WebSocketSession {
    fn push_input(&mut self, user_input: UserInput) {
        self.ism.lock().unwrap().push(user_input);
    }

    fn reject(
        &self,
        ctx: &mut actix_web_actors::ws::WebsocketContext<WebSocketSession>,
        reason: String,
    ) {
        ctx.text(
            serde_json::to_string(&ServerMessage::Reject { reason })
                .expect("Faield to serialize the Reject server message."),
        );
    }

    fn accept(
        &self,
        ctx: &mut actix_web_actors::ws::WebsocketContext<WebSocketSession>,
        player_id: i32,
    ) {
        ctx.text(
            serde_json::to_string(&ServerMessage::Accept { player_id })
                .expect("Faild to serialize the Accept server message"),
        );
    }

    fn handle_client_message(
        &mut self,
        ctx: &mut actix_web_actors::ws::WebsocketContext<WebSocketSession>,
        message: &ClientMessage,
    ) {
        match message {
            ClientMessage::Sit { player_id, name } => {
                let player_id = match player_id {
                    Some(id) => *id,
                    None => {
                        let available_ids = self.slot_manager.lock().unwrap().empty_slots();
                        if available_ids.len() == 0 {
                            self.reject(ctx, String::from("The game is full."));
                            return;
                        }
                        *available_ids.choose(&mut rand::thread_rng()).unwrap() as i32
                    }
                };

                let slot = PlayerSlot {
                    id: player_id as usize,
                    name: String::from(name),
                };

                let slot_request = self.slot_manager.lock().unwrap().request(slot.clone());
                match slot_request {
                    Err(SlotRequestError::AlreadyExists) => {
                        self.reject(ctx, format!("Chair #{} is already occupied.", player_id));
                    }
                    Err(SlotRequestError::IndexOutOfRange) => {
                        self.reject(ctx, format!("Chair #{} is out of range.", player_id));
                    }
                    Ok(()) => {
                        self.slot = Some(slot);
                        println!("Player ({}) sat the chair #{}", name, player_id);
                        self.accept(ctx, player_id);
                    }
                }
            }
            ClientMessage::Sync {} => {
                if let Some(_) = &self.slot {
                    self.push_input(UserInput {
                        player_id: self.slot.as_ref().unwrap().id as i32,
                        input: Input::RequestBroadcast,
                        token: None,
                    });
                } else {
                    self.reject(ctx, format!("No slot assigned."));
                }
            }
            ClientMessage::Input { input, token } => {
                if let Some(_) = &self.slot {
                    self.push_input(UserInput {
                        player_id: self.slot.as_ref().unwrap().id as i32,
                        input: input.clone(),
                        token: Some(*token),
                    });
                } else {
                    self.reject(ctx, format!("No slot assigned."));
                }
            }
        }
    }
}

impl Actor for WebSocketSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.ws_monitor
            .get_ref()
            .try_send(WebSocketClientRegister {
                address: _ctx.address(),
            })
            .expect("Failed to add websocket session to WebSocketSessionMonitor");
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        if let Some(slot) = &self.slot {
            self.slot_manager.lock().unwrap().remove(slot.id);
        }
        self.ws_monitor
            .get_ref()
            .try_send(WebSocketClientUnregister {
                address: _ctx.address(),
            })
            .expect("Failed to remove websocket session from WebSocketSessionMonitor");
        Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocketSession {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        let msg = match msg {
            Err(_) => {
                ctx.stop();
                return;
            }
            Ok(msg) => msg,
        };
        match msg {
            ws::Message::Ping(_) => {}
            ws::Message::Pong(_) => {}
            ws::Message::Text(text) => {
                let message = serde_json::from_str::<ClientMessage>(&text)
                    .expect("Failed to deserialize the client message.");
                self.handle_client_message(ctx, &message);
            }
            ws::Message::Binary(_) => println!("Unexpected binary"),
            ws::Message::Close(reason) => {
                ctx.close(reason);
                ctx.stop();
            }
            ws::Message::Continuation(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
    }
}

impl Handler<ArcServerMessage> for WebSocketSession {
    type Result = ();

    fn handle(&mut self, msg: ArcServerMessage, ctx: &mut Self::Context) {
        let serialized =
            &serde_json::to_string(&*msg.message).expect("Failed to serialize the server message.");
        ctx.text(serialized);
    }
}

#[derive(Clone)]
pub struct WebsocketSessionMonitor {
    addresses: HashSet<Addr<WebSocketSession>>,
    broadcast: Data<Mutex<BroadCastTarget>>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct WebSocketClientRegister {
    address: Addr<WebSocketSession>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct WebSocketClientUnregister {
    address: Addr<WebSocketSession>,
}

impl Actor for WebsocketSessionMonitor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        self.broadcast.lock().unwrap().addr = Some(ctx.address());
    }
}

impl Handler<WebSocketClientRegister> for WebsocketSessionMonitor {
    type Result = ();

    fn handle(&mut self, register: WebSocketClientRegister, _: &mut Self::Context) {
        self.addresses.insert(register.address);
    }
}

impl Handler<WebSocketClientUnregister> for WebsocketSessionMonitor {
    type Result = ();

    fn handle(&mut self, register: WebSocketClientUnregister, _: &mut Self::Context) {
        self.addresses.remove(&register.address);
    }
}

impl Handler<ArcServerMessage> for WebsocketSessionMonitor {
    type Result = ();

    fn handle(&mut self, message: ArcServerMessage, _: &mut Self::Context) {
        for addr in self.addresses.iter() {
            addr.do_send(message.clone());
        }
    }
}

async fn echo_route(
    req: HttpRequest,
    stream: web::Payload,
    ism_data: Data<Mutex<InputQueue>>,
    ws_monitor: Data<Addr<WebsocketSessionMonitor>>,
    slot_manager: Data<Mutex<PlayerSlotManager>>,
) -> Result<HttpResponse, Error> {
    println!("Connected from {:?}", req.peer_addr().unwrap());
    let session = WebSocketSession {
        ism: ism_data,
        slot_manager,
        ws_monitor,
        slot: None,
    };
    ws::start(session, &req, stream)
}

pub struct BroadCastTarget {
    pub addr: Option<Addr<WebsocketSessionMonitor>>,
}

#[actix_rt::main]
pub async fn start(
    ism: Data<Mutex<InputQueue>>,
    broadcast: Data<Mutex<BroadCastTarget>>,
    slot_manager: Data<Mutex<PlayerSlotManager>>,
) -> std::io::Result<()> {
    let ws_monitor_addr = WebsocketSessionMonitor {
        addresses: HashSet::new(),
        broadcast: broadcast.clone(),
    }
    .start();
    HttpServer::new(move || {
        App::new()
            .service(web::resource("/play/").to(echo_route))
            .app_data(ism.clone())
            .app_data(slot_manager.clone())
            .app_data(Data::new(ws_monitor_addr.clone()))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
