use super::{Input, InputQueue};
use crate::BroadCastQueue;
use actix::clock::Duration;
use actix::prelude::*;
use actix_web::web::Data;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use regex::Regex;
use std::sync::Arc;
use std::sync::Mutex;

struct EchoSession {
    ism: Data<Mutex<InputQueue>>,
    ws_monitor: Data<Addr<WebsocketSessionMonitor>>,
}

impl EchoSession {
    fn push_input(&mut self, player_id: i32, i: Input) {
        self.ism.lock().unwrap().push(player_id, i);
    }
}

impl Actor for EchoSession {
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
        Running::Stop
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for EchoSession {
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
                // ctx.text(&format!["Got {}", text]);
                let reg = Regex::new(r"([^\s]+)").unwrap();
                let mut caps = reg.find_iter(&text);
                let player_id = match caps.next() {
                    Some(pid_match) => pid_match.as_str().parse::<i32>().unwrap_or(-1),
                    None => -1,
                };
                if player_id == -1 {
                    ctx.text(format![
                        "Invalid request: Player ID is not defined {}",
                        &text
                    ]);
                    return;
                }

                match caps.next() {
                    Some(command) => match command.as_str() {
                        "Left" => self.push_input(player_id, Input::Left),
                        "Right" => self.push_input(player_id, Input::Right),
                        "Up" => self.push_input(player_id, Input::Up),
                        "Down" => self.push_input(player_id, Input::Down),
                        "RotateRight" => self.push_input(player_id, Input::RotateRight),
                        "RotateLeft" => self.push_input(player_id, Input::RotateLeft),
                        "Flip" => self.push_input(player_id, Input::Flip),
                        "GiveUp" => self.push_input(player_id, Input::GiveUp),
                        "Cancel" => self.push_input(player_id, Input::Cancel),
                        "Enter" => self.push_input(player_id, Input::Enter),
                        _ => {
                            ctx.text(format![
                                "Invalid request: unknown command {}",
                                command.as_str()
                            ]);
                        }
                    },
                    None => ctx.text(format!["Invalid request: no command"]),
                }
                // }
                // caps.at(1).unwrap()
                // self.ism.lock().unwrap().push(1, Input::Down);
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

impl Handler<BroadCast> for EchoSession {
    type Result = ();

    fn handle(&mut self, msg: BroadCast, ctx: &mut Self::Context) {
        println!("Processing Broadcast event...");
        ctx.text(format!["{}", msg.content]);
    }
}

#[derive(Clone)]
pub struct WebsocketSessionMonitor {
    addresses: Vec<Addr<EchoSession>>,
    broadcast: Data<Mutex<BroadCastTarget>>,
}

#[derive(Message)]
#[rtype(result = "()")]
struct WebSocketClientRegister {
    address: Addr<EchoSession>,
}

#[derive(MessageResponse)]
struct WebSocketClientRegistration;

#[derive(Message)]
#[rtype(result = "")]
struct ExampleMessage {
    text: String,
}

#[derive(Message)]
#[rtype(resut = "()")]
#[derive(Clone)]
pub struct BroadCast {
    pub content: Arc<String>,
}

impl Actor for WebsocketSessionMonitor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Started WebSocketSessionMonitor");
        self.broadcast.lock().unwrap().addr = Some(ctx.address());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        println!("Stopped WebSocketSessionMonitor");
    }
}

impl Handler<WebSocketClientRegister> for WebsocketSessionMonitor {
    type Result = ();

    fn handle(&mut self, register: WebSocketClientRegister, _: &mut Self::Context) {
        println!("Adding websocket session to monitor");
        self.addresses.push(register.address);
    }
}

impl Handler<BroadCast> for WebsocketSessionMonitor {
    type Result = ();

    fn handle(&mut self, broadcast: BroadCast, _: &mut Self::Context) {
        println!("Handling Broadcast event...");
        println!("Will send to {} sessions", self.addresses.len());
        for addr in self.addresses.iter() {
            println!("Sending broadcast to session");
            addr.do_send(broadcast.clone());
        }
    }
}

async fn echo_route(
    req: HttpRequest,
    stream: web::Payload,
    ism_data: Data<Mutex<InputQueue>>,
    ws_monitor: Data<Addr<WebsocketSessionMonitor>>,
) -> Result<HttpResponse, Error> {
    println!("Connected from {:?}", req.peer_addr().unwrap());
    let session = EchoSession {
        ism: ism_data,
        ws_monitor: ws_monitor,
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
) -> std::io::Result<()> {
    let ws_monitor_addr = WebsocketSessionMonitor {
        addresses: vec![],
        broadcast: broadcast.clone(),
    }
    .start();
    HttpServer::new(move || {
        App::new()
            .service(web::resource("/ws/").to(echo_route))
            .app_data(ism.clone())
            .app_data(Data::new(ws_monitor_addr.clone()))
    })
    .bind("0.0.0.0:8080")?
    .run()
    .await
}
