use super::{Input, InputQueue};
use actix::*;
use actix_web::web::Data;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use regex::Regex;
use std::sync::Mutex;

struct EchoSession {
    ism: Data<Mutex<InputQueue>>,
}

impl EchoSession {
    fn push_input(&mut self, player_id: i32, i: Input) {
        self.ism.lock().unwrap().push(player_id, i);
    }
}

impl Actor for EchoSession {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {}
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
                ctx.text(&format!["Got {}", text]);
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

async fn echo_route(
    req: HttpRequest,
    stream: web::Payload,
    ism_data: Data<Mutex<InputQueue>>,
) -> Result<HttpResponse, Error> {
    ws::start(EchoSession { ism: ism_data }, &req, stream)
}

#[actix_rt::main]
pub async fn start(ism: Data<Mutex<InputQueue>>) -> std::io::Result<()> {
    HttpServer::new(move || {
        App::new()
            .service(web::resource("/ws/").to(echo_route))
            .app_data(ism.clone())
    })
    .bind("0.0.0.0:8080")?
    .run();
    Ok(())
}
