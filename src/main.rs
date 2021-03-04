use std::sync::Mutex;
use actix_web::web::Data;
use rltk::{GameState, Point, Rltk, RGB};
use specs::prelude::*;
use clap::{App, Arg};

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

mod server;
pub use server::*;

mod input_source;
pub use input_source::*;

pub struct State {
    pub ecs: World,
    pub winner: usize,
    pub ism: Data<Mutex<InputQueue>>,
    pub my_player_id: i32,
    pub use_local_input: bool,
}

impl State {
    pub fn push_input(&mut self, player_id: i32, i: Input) {
        self.ism.lock().unwrap().push(player_id, i);
    }

    pub fn pop_for(&mut self, player_id: i32) -> Option<Input> {
        let mut input_queue = self.ism.lock().unwrap();
        input_queue.pop_for(player_id)
    }
}

#[derive(PartialEq, Copy, Clone)]
pub enum Mode {
    Initialize,
    Select,
    Put,
    Finish,
}

impl GameState for State {
    fn tick(&mut self, ctx: &mut Rltk) {
        ctx.cls();
        // ctx.print();
        let mut newmode = player_input(self, ctx);
        if self.is_finished() {
            newmode = Mode::Finish;
        }

        {
            let mut mode = self.ecs.write_resource::<Mode>();
            self.winner = if *mode != newmode {
                let mut stats = StatsCollectSystem { winner: 0 };
                stats.run_now(&self.ecs);
                stats.winner
            } else {
                self.winner
            };

            let dialogs = match *mode {
                Mode::Initialize => vec![],
                Mode::Select => {
                    vec!["Left/Right: Select a piece to put  Enter: Put  Num0: Give up".to_string()]
                }
                Mode::Put => vec![
                    "Left/Right/Up/Down: Move a piece  Enter: Put  Num0: Give up".to_string(),
                    "R: Rotate right  E: Rotate left  F: Flip  Esc: Cancel".to_string(),
                ],
                Mode::Finish => vec![format!["Player #{} won!", self.winner + 1]],
            };
            for (i, dialog) in dialogs.iter().enumerate() {
                ctx.print(5, i * 2 + 60, dialog);
            }

            *mode = newmode;
        }

        let positions = self.ecs.read_storage::<Position>();
        let players = self.ecs.read_storage::<Player>();
        let active_player_id = self.ecs.fetch::<usize>();
        let polynomios = self.ecs.read_storage::<Polynomio>();
        let rects = self.ecs.read_storage::<Rect>();
        let map = self.ecs.read_resource::<Map>();

        draw_map(ctx, &*map);
        for (pos, rect) in (&positions, &rects).join() {
            draw_rect(ctx, pos, rect);
        }
        for (pos, player) in (&positions, &players).join() {
            draw_ui(ctx, pos, player, *active_player_id);
        }
        for (pos, polynomio) in (&positions, &polynomios).join() {
            draw_polynomio(ctx, pos, polynomio);
        }

        let players = self.ecs.read_storage::<Player>();
        let active_player_id = self.ecs.read_resource::<usize>();
        let player = players
            .get(self.ecs.fetch::<Vec<Entity>>()[*active_player_id])
            .unwrap();

        let active_position = positions.get(player.polynomios[player.select]).unwrap();
        let active_polynomio = polynomios.get(player.polynomios[player.select]).unwrap();

        draw_polynomio(ctx, active_position, active_polynomio);

        if newmode != Mode::Finish {
            let upper_left = polynomios
                .get(player.polynomios[player.select])
                .unwrap()
                .upper_left();
            ctx.set(
                active_position.x + upper_left.x - 1,
                active_position.y + upper_left.y,
                RGB::named(rltk::WHITE),
                RGB::named(rltk::BLACK),
                rltk::to_cp437('>'),
            );
        }
    }
}


fn main() -> rltk::BError {
    let ism: Data<Mutex<InputQueue>> = Data::new(
        Mutex::new(InputQueue::new()));
    start(ism.clone());

    let matches = App::new("Blokus")
                          .version("1.0")
                          .author("Ryu Wakimoto")
                          .about("Blokus implementation in Rust with Rltk")
                          .arg(Arg::with_name("mode")
                               .short("m")
                               .long("mode")
                               .help("Game mode. 'normal': 4-players game 'duo': 2-players game")
                               .possible_values(&["normal", "duo"])
                               .takes_value(true))
                          .arg(Arg::with_name("type")
                               .short("t")
                               .long("type")
                               .possible_values(&["local", "host", "client"])
                               .takes_value(true))
                          .arg(Arg::with_name("player-id")
                               .short("p")
                               .long("player-id")
                                .takes_value(true))
                          .get_matches();
    
    let game_mode = matches.value_of("mode").unwrap_or("normal");
    let instance_type = matches.value_of("type").unwrap_or("local");
    let my_player_id = matches.value_of("player-id").unwrap_or("0").parse::<i32>().unwrap_or(0);
    let use_local_input = instance_type == "local";

    use rltk::RltkBuilder;
    let context = RltkBuilder::simple(72, 64)?.with_title("Blokus").build()?;

    let mut gs = State {
        ecs: World::new(),
        winner: 0,
        ism: ism.clone(),
        my_player_id: my_player_id,
        use_local_input: use_local_input,
    };
    gs.ecs.register::<Position>();
    gs.ecs.register::<Polynomio>();
    gs.ecs.register::<Player>();
    gs.ecs.register::<Rect>();

    match game_mode {
        "normal" => gs.prepare_4players_game(),
        "duo" => gs.prepare_2players_game(),
        _ => {}
    };

    rltk::main_loop(context, gs)
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
            let player_comps: Vec<&Player> = players.iter().map(|e| players_store.get(*e).unwrap()).collect();
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
            let player_comps: Vec<&Player> = players.iter().map(|e| players_store.get(*e).unwrap()).collect();
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
            .with(Polynomio::new(coods_vec.clone(), color * 0.2))
            .build();

        self.ecs
            .create_entity()
            .with(Position::new(x, y))
            .with(Polynomio::new(coods_vec, color))
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
            .build();
        return player;
    }
}
