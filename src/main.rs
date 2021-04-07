use actix_web::web::Data;
use clap::{App, AppSettings, Arg, SubCommand};
use rltk::{GameState, Point, Rltk, RGB};
use specs::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;
use specs::saveload::{SimpleMarker, SimpleMarkerAllocator, MarkedBuilder};
use serde::{Serialize, Deserialize};
use rand::prelude::*;

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

mod client;
pub use client::*;


pub struct State {
    pub ecs: World,
    pub winner: usize,
    pub ism: Data<Mutex<InputQueue>>,
    pub my_player_id: i32,
    pub use_local_input: bool,
    pub event_history: Vec<Box<dyn Event>>,
    pub broadcast: Option<Data<Mutex<BroadCastTarget>>>,
    pub slot_manager: Option<Data<Mutex<PlayerSlotManager>>>,
    pub pending_broadcast: bool,
}

impl State {
    pub fn new(
        game_mode: &str,
        ism: Data<Mutex<InputQueue>>,
        my_player_id: i32,
        use_local_input: bool,
        broadcast: Option<Data<Mutex<BroadCastTarget>>>,
        slot_manager: Option<Data<Mutex<PlayerSlotManager>>>,
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
            slot_manager,
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
            "debug" => state.prepare_game_small(),
            _ => {}
        };

        state
    }

    pub fn push_input(&mut self, user_input: UserInput) {
        self.ism.lock().unwrap().push(user_input);
    }

    pub fn pop_input(&mut self) -> Option<UserInput> {
        self.ism.lock().unwrap().pop()
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

    pub fn broadcast(&mut self, trigger: UserInput) {
        if let Some(broadcast) = &self.broadcast {
            if let Some(addr) = &broadcast.lock().unwrap().addr {
                addr.try_send(ArcServerMessage{
                    message: Arc::new(ServerMessage::Sync{
                        serialized_data: dump_game(&mut self.ecs),
                        trigger,
                    })})
                .expect("Failed to broadcast");
            }
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
        if let Some(slot_maneger) = &self.slot_manager {
            let mut sm = slot_maneger.lock().unwrap();
            if sm.consume_updated() {
                let mut players = self.ecs.write_storage::<Player>();
                let entities = self.ecs.entities();
                for (_, player) in (&entities, & mut players).join() {
                    if let Some(slot) = sm.get(player.id as usize) {
                        player.name = Some(slot.name.clone())
                    } else {
                        player.name = None
                    }
                }
            }
        }

        let initializing = {
            let mut mode = self.ecs.write_resource::<Mode>();
            if *mode == Mode::Initialize {
                *mode = Mode::Select;
                true
            } else { false }
        };

        if initializing {
            let mut stats = StatsCollectSystem { winner: 0 };
            stats.run_now(&self.ecs);
            render(&self.ecs, ctx, self.slot_manager.clone());
        }

        let active_player_id = *self.ecs.read_resource::<usize>() as i32;

        let host_player_id = if self.use_local_input {active_player_id} else {self.my_player_id};
        map_virtual_key_code(ctx.key).map(|i| self.push_input(
            UserInput {
                player_id: host_player_id,
                input: i,
                token: Some(0),
            }
        ));

        let mut input_result: InputResult = InputResult::Noop;

        loop {
            let user_input = self.pop_input();
            if let Some(user_input) = user_input {
                println!("Input: {:?}", user_input);
                input_result = player_input(self, user_input);
                println!("  --> {:?}", input_result);
                match input_result.clone() {
                    InputResult::Updated {..} => {input_result = input_result},
                    InputResult::Noop => { continue; }
                }
            }
            break;
        }

        let mut polynomio_indexing_system = PolynomioIndexingSystem {};
        polynomio_indexing_system.run_now(&self.ecs);

        match input_result {
            InputResult::Updated {newmode, trigger} => {
                {
                    let mut mode = self.ecs.write_resource::<Mode>();
                    *mode = newmode;
                }

                let mut stats = StatsCollectSystem { winner: 0 };
                stats.run_now(&self.ecs);
                self.winner = stats.winner;

                render(&self.ecs, ctx, self.slot_manager.clone());

                if let Some(trigger) = trigger {
                    self.broadcast(trigger);
                }
            }
            _ => {}
        }
    }
}

pub struct ClientState {
    pub ecs: World,
    pub url: String,
    pub player_id: i32,
    pub player_name: String,
    pub client: Client,
    pub rnd: ThreadRng,
    pub latest_token: i32,
    pub locked: bool,
}

impl ClientState {
    fn new(url: String, player_id: i32, player_name: String) -> Self {

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

        let client = Client::new(url.clone(), player_id, player_name.clone());

        if let Err(err) = client {
            eprintln!("[ERROR] Failed to connect the server (\"{}\"): {:?}", url.clone(), err);
            std::process::exit(1);
        }

        let mut client = client.unwrap();

        client.send_sit();
        client.send_sync();

        ClientState {
            ecs: ecs,
            url: url.clone(),
            player_id,
            player_name,
            client,
            rnd: thread_rng(),
            latest_token: 0,
            locked: false,
        }
    }
}

impl GameState for ClientState {
    fn tick(&mut self, ctx: &mut rltk::Rltk) {
        map_virtual_key_code(ctx.key).map(|i| {
            if i != Input::Undo && !self.locked{
                let token: i32 = self.rnd.gen();
                self.latest_token = token;
                self.client.send_input(i, token);
            }
        });
        
        let newmode = player_input_client(self, ctx);
        {
            let mut mode = self.ecs.write_resource::<Mode>();
            *mode = newmode
        }

        match self.client.next_message() {
            Some(message) => match message {
                ServerMessage::Reject { reason } => {
                    eprintln!("[ERROR] Rejected from the server: {:?}. Exiting ...", reason);
                    std::process::exit(1);
                },
                ServerMessage::Sync { serialized_data, trigger } => {
                    if trigger.player_id != self.player_id || trigger.input == Input::RequestBroadcast || trigger.token == Some(self.latest_token) {
                        println!("Applying a game update...");
                        load_game(&mut self.ecs, &serialized_data);
                        println!("Applied the game update: mode: {:?}, apid: {:?}",
                            *self.ecs.fetch::<Mode>(),
                            *self.ecs.fetch::<usize>());
                        self.locked = false;
                    }
                }
            }
            None => {}
        }

        render(&self.ecs, ctx, None);
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
                .possible_values(&["normal", "duo", "debug"])
                .takes_value(true),
        )
        .arg(
            Arg::with_name("name")
                .short("n")
                .long("name")
                .help("Player name. Default: Anonymous")
                .takes_value(true)
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
    let name = matches.value_of("name").unwrap_or("Anonymous");

    if name.len() <= 0 || name.len() > 30 {
        eprintln!("[ERROR] The length of the name must be greater than 1 and less than 31");
        std::process::exit(1);
    }

    let valid_name_pattern = regex::Regex::new(r"^[A-z_\-#@$%*,]+$").unwrap();
    if !valid_name_pattern.is_match(name) {
        eprintln!("[ERROR] The name must match ^[A-z_-#@$%*,]+$");
        std::process::exit(1);
    }


    use rltk::RltkBuilder;
    let context = RltkBuilder::simple(72, 64)?.with_title("Blokus").build()?;

    if let Some(_) = matches.subcommand_matches("play") {
        let ism: Data<Mutex<InputQueue>> = Data::new(Mutex::new(InputQueue::new()));
        let gs = State::new(game_mode, ism, 0, true, None, None);

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

        let n_players = match game_mode {
            "duo" => 2,
            "normal" => 4,
            _ => 4,
        } as usize;
        let slot_manager = Data::new(Mutex::new(PlayerSlotManager::new(n_players)));
        slot_manager.lock().unwrap().request(
            PlayerSlot {id: my_player_id as usize, name: String::from(name)})
            .expect("Failed to allocate a slot");

        let ism_ref = ism.clone();
        let broadcast_ref = broadcast.clone();
        let slot_manager_ref = slot_manager.clone();

        std::thread::spawn(move || {
            start(ism_ref, broadcast_ref, slot_manager)
                .expect("Failed to run a server");
            std::process::exit(0);
        });

        let gs = State::new(
            game_mode,
            ism,
            my_player_id,
            false,
            Some(broadcast),
            Some(slot_manager_ref));

        rltk::main_loop(context, gs)
    } else if let Some(ref sub_matches) = matches.subcommand_matches("join") {
        let my_player_id = sub_matches
            .value_of("player-id")
            .unwrap_or("0")
            .parse::<i32>()
            .unwrap_or(0);
        let url = sub_matches.value_of("url").unwrap_or("localhost:8080/ws/");

        let gs = ClientState::new(url.to_string(), my_player_id, String::from(name));
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
            self.prepare_player_small(1, 5, 44, RGB::from_f32(1.0, 0.9, 0.2)),
        ];

        let mut map = Map::new(27, 20, 7, 7);
        {
            let players_store = self.ecs.read_storage::<Player>();
            let player_comps: Vec<&Player> = players
                .iter()
                .map(|e| players_store.get(*e).unwrap())
                .collect();
            map.bind(player_comps[0], 1, 1);
            map.bind(player_comps[1], 5, 5);
        }

        self.ecs.insert(players);
        self.ecs.insert(map);
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
            .with(Player::new(id, ps, color, None))
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

        ps.push(self.prepare_polynomio(x + 5, y + 2 + YOFF, &[(0, 0)], color));

        let player = self
            .ecs
            .create_entity()
            .with(Player::new(id, ps, color, None))
            .with(Position::new(x, y))
            .marked::<SimpleMarker<SyncOnline>>()
            .build();
        return player;
    }
}
