use actix_web::web::Data;
use clap::{App, AppSettings, Arg, SubCommand};
use rltk::{GameState, Point, Rltk, RGB};
use specs::prelude::*;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use websocket::ClientBuilder;
use websocket::Message;
use specs::saveload::{SimpleMarker, SimpleMarkerAllocator, MarkedBuilder};
use serde::{Serialize, Deserialize};

mod components;
pub use components::*;

mod player;
pub use player::*;

mod map;
pub use map::*;

mod render;
pub use render::*;

mod stats_collect_system;
pub use stats_collect_system::*;

mod polynomio_indexing_system;
pub use polynomio_indexing_system::*;

mod server;
pub use server::*;

mod input_source;
pub use input_source::*;

mod broadcast;
pub use broadcast::*;

mod save_load_system;
pub use save_load_system::*;

mod events;
pub use events::*;

mod entity_vec;
pub use entity_vec::*;

pub struct State {
    pub ecs: World,
    pub winner: usize,
    pub ism: Data<Mutex<InputQueue>>,
    pub my_player_id: i32,
    pub use_local_input: bool,
    pub event_history: Vec<Box<dyn Event>>,
    pub broadcast: Option<Data<Mutex<BroadCastTarget>>>,
    pub pending_broadcast: bool,
}

impl State {
    pub fn new(
        game_mode: &str,
        ism: Data<Mutex<InputQueue>>,
        my_player_id: i32,
        use_local_input: bool,
        broadcast: Option<Data<Mutex<BroadCastTarget>>>,
    ) -> Self {
        let mut state = State {
            ecs: World::new(),
            winner: 0,
            ism: ism.clone(),
            my_player_id: my_player_id,
            use_local_input: use_local_input,
            event_history: Vec::new(),
            broadcast: broadcast,
            pending_broadcast: false,
        };
        state.ecs.register::<Position>();
        state.ecs.register::<Polynomio>();
        state.ecs.register::<Player>();
        state.ecs.register::<Rect>();
        state.ecs.register::<SimpleMarker<SyncOnline>>();
        state.ecs.register::<SerializeHelper>();

        state.ecs.insert(SimpleMarkerAllocator::<SyncOnline>::new());

        match game_mode {
            "normal" => state.prepare_4players_game(),
            "duo" => state.prepare_2players_game(),
            _ => {}
        };

        state
    }

    pub fn push_input(&mut self, player_id: i32, i: Input) {
        self.ism.lock().unwrap().push(player_id, i);
    }

    pub fn pop_for(&mut self, player_id: i32) -> Option<Input> {
        let mut input_queue = self.ism.lock().unwrap();
        input_queue.pop_for(player_id)
    }

    pub fn push_event(&mut self, event: Box<dyn Event>) {
        self.event_history.push(event);
    }

    pub fn undo(&mut self) {
        if let Some(mut event) = self.event_history.pop() {
            (*event).undo(self);
            if event.should_chain_next() {
                self.undo();
            }
        }
    }

    pub fn broadcast(&mut self) {
        if let Some(broadcast) = &self.broadcast {
            if let Some(addr) = &broadcast.lock().unwrap().addr {
                addr.try_send(BroadCast {
                    content: Arc::new(dump_game(&mut self.ecs)),
                })
                .expect("Failed to broadcast");
            }
        }
    }

    pub fn request_broadcast(&mut self) {
        self.pending_broadcast = true;
    }

    pub fn consume_broadcast(&mut self) {
        let broadcast_request_from_clients = {
            let mut input_queue = self.ism.lock().unwrap();
            input_queue.consume_broadcast()
        };

        if self.pending_broadcast || broadcast_request_from_clients{
            self.broadcast();
            self.pending_broadcast = false;
        }
    }
}

#[derive(PartialEq, Copy, Clone, Serialize, Deserialize, Debug)]
pub enum Mode {
    Initialize,
    Select,
    Put,
    Finish,
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {

        let mut newmode = player_input(self, ctx);
        if self.is_finished() {
            newmode = Mode::Finish;
        }

        let mut polynomio_indexing_system = PolynomioIndexingSystem {};
        polynomio_indexing_system.run_now(&self.ecs);

        let currentmode;
        {
            let mut mode = self.ecs.write_resource::<Mode>();

            self.winner = if *mode != newmode {
                let mut stats = StatsCollectSystem { winner: 0 };
                stats.run_now(&self.ecs);
                stats.winner
            } else {
                self.winner
            };

            currentmode = *mode;
            *mode = newmode;
        }

        if currentmode != newmode {
            self.request_broadcast();
        }

        render(&self.ecs, ctx);

        self.consume_broadcast();
    }
}

struct ClientState {
    pub ecs: World,
    pub url: String,
    pub player_id: i32,
    pub websocket_sender: websocket::sender::Writer<std::net::TcpStream>,
    pub message_queue: Arc<Mutex<VecDeque<String>>>,
}

impl ClientState {
    fn new(url: String, player_id: i32) -> Self {
        let msg_queue = Arc::new(Mutex::new(VecDeque::new()));
        let client = ClientBuilder::new(&url)
            .unwrap()
            .connect_insecure()
            .unwrap();
        let (mut receiver, mut sender) = client.split().unwrap();

        let msg_queue_thread = msg_queue.clone();

        let mut ecs = World::new();

        ecs.register::<Position>();
        ecs.register::<Polynomio>();
        ecs.register::<Player>();
        ecs.register::<SimpleMarker<SyncOnline>>();
        ecs.register::<SerializeHelper>();

        let empty_players: Vec<Entity> = Vec::new();
        ecs.insert(SimpleMarkerAllocator::<SyncOnline>::new());
        ecs.insert(Map::new(0, 0, 1, 1));
        ecs.insert(empty_players);
        ecs.insert(0 as usize);
        ecs.insert(Mode::Initialize);

        std::thread::spawn(move || {
            for message in receiver.incoming_messages() {
                match message {
                    Ok(websocket::OwnedMessage::Text(text)) => {
                        msg_queue_thread.lock().unwrap().push_back(text);
                    },
                    Ok(_) => {},
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

        sender.send_message(&Message::text(
            format!("{} {}", player_id, "RequestBroadcast"))).unwrap();
        ClientState {
            ecs: ecs,
            url: url.clone(),
            player_id,
            websocket_sender: sender,
            message_queue: msg_queue,
        }
    }
}

impl GameState for ClientState {
    fn tick(&mut self, ctx: &mut rltk::Rltk) {
        map_virtual_key_code(ctx.key).map(|i| {
            let keytext = match i {
                Input::RotateRight => "RotateRight",
                Input::RotateLeft => "RotateLeft",
                Input::Flip => "Flip",
                Input::Up => "Up",
                Input::Down => "Down",
                Input::Left => "Left",
                Input::Right => "Right",
                Input::Enter => "Enter",
                Input::Cancel => "Cancel",
                Input::GiveUp => "GiveUp",
                Input::RequestBroadcast => "",
            };
            let message = format!("{} {}", self.player_id, keytext);
            println!("Sending: {}", message);
            self.websocket_sender
                .send_message(&Message::text(message))
                .unwrap();
        });

        let mut queue = self.message_queue.lock().unwrap();
        while !queue.is_empty() {
            let data = &queue.pop_front().unwrap();
            load_game(&mut self.ecs, data);
            println!("Received game update: mode: {:?}, apid: {:?}",
                *self.ecs.fetch::<Mode>(),
                *self.ecs.fetch::<usize>());
            render(&self.ecs, ctx);
        }
    }
}

fn main() -> rltk::BError {
    let matches = App::new("Blokus")
        .version("1.0")
        .author("Ryu Wakimoto")
        .about("Blokus implementation in Rust with Rltk")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::with_name("mode")
                .short("m")
                .long("mode")
                .help("Game mode. 'normal': 4-players game 'duo': 2-players game")
                .possible_values(&["normal", "duo"])
                .takes_value(true),
        )
        .subcommand(SubCommand::with_name("play"))
        .subcommand(
            SubCommand::with_name("host").arg(
                Arg::with_name("player-id")
                    .short("p")
                    .long("player-id")
                    .takes_value(true),
            ),
        )
        .subcommand(
            SubCommand::with_name("join")
                .arg(Arg::with_name("url").required(true).takes_value(true))
                .arg(
                    Arg::with_name("player-id")
                        .short("p")
                        .long("player-id")
                        .required(true)
                        .takes_value(true),
                ),
        )
        .get_matches();

    let game_mode = matches.value_of("mode").unwrap_or("normal");

    use rltk::RltkBuilder;
    let context = RltkBuilder::simple(72, 64)?.with_title("Blokus").build()?;

    if let Some(_) = matches.subcommand_matches("play") {
        let ism: Data<Mutex<InputQueue>> = Data::new(Mutex::new(InputQueue::new()));
        let gs = State::new(game_mode, ism, 0, true, None);

        rltk::main_loop(context, gs)
    } else if let Some(ref sub_matches) = matches.subcommand_matches("host") {
        let my_player_id = sub_matches
            .value_of("player-id")
            .unwrap_or("0")
            .parse::<i32>()
            .unwrap_or(0);

        let ism: Data<Mutex<InputQueue>> = Data::new(Mutex::new(InputQueue::new()));
        let broadcast: Data<Mutex<BroadCastTarget>> =
            Data::new(Mutex::new(BroadCastTarget { addr: None }));

        let ism_ref = ism.clone();
        let broadcast_ref = broadcast.clone();

        std::thread::spawn(move || {
            start(ism_ref, broadcast_ref);
            std::process::exit(0);
        });

        let gs = State::new(game_mode, ism, my_player_id, false, Some(broadcast));
        rltk::main_loop(context, gs)
    } else if let Some(ref sub_matches) = matches.subcommand_matches("join") {
        let my_player_id = sub_matches
            .value_of("player-id")
            .unwrap_or("0")
            .parse::<i32>()
            .unwrap_or(0);
        let url = sub_matches.value_of("url").unwrap_or("localhost:8080/ws/");

        let gs = ClientState::new(url.to_string(), my_player_id);
        rltk::main_loop(context, gs)
    } else {
        panic!("Unknown subcommand");
    }
}

impl State {
    pub fn change_mode(&mut self, m: Mode) {
        let mut mode = self.ecs.fetch_mut::<Mode>();
        *mode = m;
    }

    pub fn next_player(&mut self) {
        let mut active_player_id = self.ecs.fetch_mut::<usize>();
        let player_entities = self.ecs.fetch::<Vec<Entity>>();
        let players = self.ecs.read_storage::<Player>();

        let mut next_player_id = *active_player_id;
        for _ in 0..player_entities.len() {
            next_player_id = (next_player_id + 1) % player_entities.len();
            if !players.get(player_entities[next_player_id]).unwrap().end {
                *active_player_id = next_player_id;
                return;
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        let player_entities = self.ecs.fetch::<Vec<Entity>>();
        let players = self.ecs.read_storage::<Player>();
        let mut finished = true;
        for e in player_entities.iter() {
            finished &= players.get(*e).unwrap().end;
        }
        finished
    }

    fn prepare_4players_game(&mut self) {
        let players = vec![
            self.prepare_player(0, 5, 2, RGB::from_f32(1.0, 0.25, 0.2)),
            self.prepare_player(1, 5, 10, RGB::from_f32(0.2, 1.0, 0.2)),
            self.prepare_player(2, 5, 44, RGB::from_f32(1.0, 0.9, 0.2)),
            self.prepare_player(3, 5, 52, RGB::from_f32(0.2, 0.7, 1.0)),
        ];

        let mut map = Map::new(27, 20, 22, 22);
        {
            let players_store = self.ecs.read_storage::<Player>();
            let player_comps: Vec<&Player> = players
                .iter()
                .map(|e| players_store.get(*e).unwrap())
                .collect();
            map.bind_left_top(player_comps[0]);
            map.bind_right_top(player_comps[1]);
            map.bind_right_bottom(player_comps[2]);
            map.bind_left_bottom(player_comps[3]);
        }

        let data = serde_json::to_string(&map).unwrap();
        println!("{:?}", data);

        self.ecs.insert(players);
        self.ecs.insert(map);
        self.ecs.insert(0 as usize);
        self.ecs.insert(Mode::Initialize);
    }

    fn prepare_2players_game(&mut self) {
        let players = vec![
            self.prepare_player(0, 5, 7, RGB::from_f32(1.0, 0.25, 0.2)),
            self.prepare_player(1, 5, 47, RGB::from_f32(1.0, 0.9, 0.2)),
        ];

        let mut map = Map::new(30, 23, 16, 16);
        {
            let players_store = self.ecs.read_storage::<Player>();
            let player_comps: Vec<&Player> = players
                .iter()
                .map(|e| players_store.get(*e).unwrap())
                .collect();
            map.bind(player_comps[0], 5, 5);
            map.bind(player_comps[1], 10, 10);
        }

        self.ecs.insert(players);
        self.ecs.insert(map);
        self.ecs.insert(0 as usize);
        self.ecs.insert(Mode::Initialize);
    }

    #[allow(dead_code)]
    fn prepare_game_small(&mut self) {
        let players = vec![
            self.prepare_player_small(0, 5, 2, RGB::from_f32(1.0, 0.25, 0.2)),
            self.prepare_player_small(1, 5, 10, RGB::from_f32(0.2, 1.0, 0.2)),
            self.prepare_player_small(2, 5, 44, RGB::from_f32(1.0, 0.9, 0.2)),
            self.prepare_player_small(3, 5, 52, RGB::from_f32(0.2, 0.7, 1.0)),
        ];

        self.ecs.insert(players);
        self.ecs.insert(Map::new(27, 20, 7, 7));
        self.ecs.insert(0 as usize);
        self.ecs.insert(Mode::Initialize);
    }

    fn prepare_polynomio(&mut self, x: i32, y: i32, coods: &[(i32, i32)], color: RGB) -> Entity {
        let mut coods_vec = Vec::new();
        for cood in coods {
            coods_vec.push(Point::new(cood.0, cood.1));
        }
        self.ecs
            .create_entity()
            .with(Position::new(x, y))
            .with(Polynomio::new(coods_vec.clone(), color * 0.2, true))
            .marked::<SimpleMarker<SyncOnline>>()
            .build();

        self.ecs
            .create_entity()
            .with(Position::new(x, y))
            .with(Polynomio::new(coods_vec, color, false))
            .marked::<SimpleMarker<SyncOnline>>()
            .build()
    }

    fn prepare_player(&mut self, id: i32, x: i32, y: i32, color: RGB) -> Entity {
        let mut ps = Vec::new();
        const YOFF: i32 = 2;

        ps.push(self.prepare_polynomio(
            x,
            y + 2 + YOFF,
            &[(0, -2), (0, -1), (0, 0), (0, 1), (0, 2)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 2,
            y + 2 + YOFF,
            &[(0, -2), (0, -1), (0, 0), (0, 1), (1, -2)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 5,
            y + 2 + YOFF,
            &[(0, -2), (0, -1), (0, 0), (0, 1), (1, -1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 8,
            y + 2 + YOFF,
            &[(1, -2), (0, -1), (0, 0), (0, 1), (1, -1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 12,
            y + 2 + YOFF,
            &[(-1, -1), (0, -1), (1, -1), (-1, 0), (-1, 1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 16,
            y + 2 + YOFF,
            &[(-1, 1), (-1, 0), (0, 0), (1, 0), (1, -1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 20,
            y + 2 + YOFF,
            &[(-1, 1), (-1, 0), (0, 0), (0, -1), (1, -1)],
            color,
        ));
        // +
        ps.push(self.prepare_polynomio(
            x + 24,
            y + 2 + YOFF,
            &[(-1, 0), (0, -1), (0, 0), (1, 0), (0, 1)],
            color,
        ));

        // [
        ps.push(self.prepare_polynomio(
            x + 27,
            y + 2 + YOFF,
            &[(0, -1), (0, 0), (0, 1), (1, -1), (1, 1)],
            color,
        ));

        // T
        ps.push(self.prepare_polynomio(
            x + 31,
            y + 2 + YOFF,
            &[(0, 0), (0, -1), (0, 1), (-1, 1), (1, 1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 34,
            y + 2 + YOFF,
            &[(0, 0), (0, 1), (0, -1), (1, 0), (1, 1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 38,
            y + 2 + YOFF,
            &[(0, 0), (-1, 0), (0, -1), (0, 1), (1, -1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 42,
            y + 2 + YOFF,
            &[(0, 0), (-1, 0), (-1, -1), (0, 1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 44,
            y + 2 + YOFF,
            &[(0, 0), (0, -1), (0, 1), (1, -1)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 47,
            y + 2 + YOFF,
            &[(0, 0), (0, -1), (0, 1), (1, 0)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 50,
            y + 1 + YOFF,
            &[(0, -1), (0, 0), (0, 1), (0, 2)],
            color,
        ));

        ps.push(self.prepare_polynomio(
            x + 52,
            y + 1 + YOFF,
            &[(0, 0), (1, 0), (0, 1), (1, 1)],
            color,
        ));

        ps.push(self.prepare_polynomio(x + 55, y + 2 + YOFF, &[(0, -1), (0, 0), (0, 1)], color));

        ps.push(self.prepare_polynomio(x + 58, y + 1 + YOFF, &[(-1, 0), (0, 0), (0, 1)], color));

        ps.push(self.prepare_polynomio(x + 60, y + 2 + YOFF, &[(0, 0), (0, 1)], color));

        ps.push(self.prepare_polynomio(x + 62, y + 2 + YOFF, &[(0, 0)], color));

        let player = self
            .ecs
            .create_entity()
            .with(Player::new(id, ps, color))
            .with(Position::new(x, y))
            .marked::<SimpleMarker<SyncOnline>>()
            .build();
        return player;
    }

    #[allow(dead_code)]
    fn prepare_player_small(&mut self, id: i32, x: i32, y: i32, color: RGB) -> Entity {
        let mut ps = Vec::new();
        const YOFF: i32 = 2;

        ps.push(self.prepare_polynomio(
            x + 2,
            y + 2 + YOFF,
            &[(0, -2), (0, -1), (0, 0), (0, 1), (1, -2)],
            color,
        ));

        ps.push(self.prepare_polynomio(x + 65, y + 2 + YOFF, &[(0, 0)], color));

        let player = self
            .ecs
            .create_entity()
            .with(Player::new(id, ps, color))
            .with(Position::new(x, y))
            .marked::<SimpleMarker<SyncOnline>>()
            .build();
        return player;
    }
}
