use crate::dump_game;
use super::{
    GiveUpEvent, Input, Map, Mode, Player, Polynomio, Position, PutEvent, State, TurnChangeEvent,
};
use rltk::{Point, Rltk, VirtualKeyCode};
use specs::Entity;
use specs::WorldExt;

pub fn player_input(gs: &mut State, ctx: &mut Rltk) -> Mode {
    let mode = *gs.ecs.fetch::<Mode>();
    if ctx.key == Some(VirtualKeyCode::Z) {
        gs.undo();
        return mode;
    }

    let active_player_id = *gs.ecs.read_resource::<usize>();
    let player_id = if gs.use_local_input {
        active_player_id as i32
    } else {
        gs.my_player_id
    };

    map_virtual_key_code(ctx.key).map(|i| gs.push_input(player_id, i));

    match mode {
        Mode::Initialize => return Mode::Select,
        Mode::Select => player_input_select(gs),
        Mode::Put => player_input_put(gs),
        Mode::Finish => return Mode::Finish,
    }
}

fn player_input_select(gs: &mut State) -> Mode {
    let player_entity: Entity;
    let active_player_id: usize;
    let mut ended = false;
    let mut request_broadcast = false;
    {
        active_player_id = *gs.ecs.read_resource::<usize>();
        player_entity = gs.ecs.fetch::<Vec<Entity>>()[active_player_id];
        let input = gs.pop_for(active_player_id as i32);
        let map = gs.ecs.write_resource::<Map>();
        let mut players = gs.ecs.write_storage::<Player>();
        let player = players.get_mut(player_entity).unwrap();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut active_position = positions.get_mut(player.polynomios[player.select]).unwrap();

        match input {
            None => {}
            Some(key) => {
                request_broadcast = true;
                match key {
                    Input::Right => {
                        select_next(player, false);
                    }
                    Input::Left => {
                        select_next(player, true);
                    }
                    Input::Enter => {
                        active_position.x = map.width as i32 / 2 + map.x;
                        active_position.y = map.height as i32 / 2 + map.y;
                        return Mode::Put;
                    }
                    Input::GiveUp => {
                        player.end = true;
                        ended = true;
                    }
                    _ => {}
                }
            },
        }
    }
    if ended {
        gs.next_player();
        gs.push_event(Box::new(GiveUpEvent {
            player_entity: player_entity,
        }));
        gs.push_event(Box::new(TurnChangeEvent {
            from: active_player_id,
        }));
    }
    if request_broadcast {
        gs.request_broadcast();
    }
    Mode::Select
}

fn player_input_put(gs: &mut State) -> Mode {
    let mut next_player = false;
    let mut ended = false;
    let mut request_broadcast = false;
    let active_player_id: usize;
    let player_entity;
    let player_select;
    let player;
    
    {
        active_player_id = *gs.ecs.read_resource::<usize>();
        player_entity = gs.ecs.fetch::<Vec<Entity>>()[active_player_id];
        let input = gs.pop_for(active_player_id as i32);
        let mut map = gs.ecs.write_resource::<Map>();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut polynomios = gs.ecs.write_storage::<Polynomio>();
        let mut players = gs.ecs.write_storage::<Player>();

        player = players.get_mut(player_entity).unwrap();
        player_select = player.select;
        let active_position = positions.get_mut(player.polynomios[player.select]).unwrap();
        let active_polynomio = polynomios
            .get_mut(player.polynomios[player.select])
            .unwrap();

        match input {
            None => {}
            Some(key) => {
                request_broadcast = true;
                match key {
                    Input::RotateRight => {
                        active_polynomio.rotate(true);
                    }
                    Input::RotateLeft => {
                        active_polynomio.rotate(false);
                    }
                    Input::Flip => {
                        active_polynomio.flip();
                    }
                    Input::Up => {
                        active_position.translate_within(0, -1, &*map);
                    }
                    Input::Down => {
                        active_position.translate_within(0, 1, &*map);
                    }
                    Input::Left => {
                        active_position.translate_within(-1, 0, &*map);
                    }
                    Input::Right => {
                        active_position.translate_within(1, 0, &*map);
                    }
                    Input::Enter => {
                        active_position.to_point();
                        let put_to = Point::new(active_position.x - map.x, active_position.y - map.y);

                        if map.try_put(put_to, active_polynomio, active_player_id as i32) {
                            player.fixed[player.select] = true;
                            if !select_next(player, false) {
                                player.end = true;
                                ended = true;
                            }
                            next_player = true;
                        }
                    }
                    Input::Cancel => {
                        active_position.reset();
                        active_polynomio.reset();
                        return Mode::Select;
                    }
                    _ => {}
                }
            },
        }
    }
    if next_player {
        gs.next_player();
        gs.push_event(Box::new(PutEvent {
            player_entity: player_entity,
            polynomio_id: player_select,
        }));
        gs.push_event(Box::new(TurnChangeEvent {
            from: active_player_id,
        }));
        if ended {
            gs.push_event(Box::new(GiveUpEvent {
                player_entity: player_entity,
            }));
        }
        return Mode::Select;
    }
    if request_broadcast {
        gs.request_broadcast();
    }
    Mode::Put
}

fn select_next(player: &mut Player, reverse: bool) -> bool {
    let n = player.polynomios.len();
    let delta = if reverse { n - 1 } else { 1 };
    player.select += delta;
    player.select %= n;

    for _ in 0..n {
        if !player.fixed[player.select] {
            return true;
        }
        player.select += delta;
        player.select %= n;
    }

    return false;
}

pub fn map_virtual_key_code(key: Option<VirtualKeyCode>) -> Option<Input> {
    match key {
        None => None,
        Some(key) => match key {
            VirtualKeyCode::R => Some(Input::RotateRight),
            VirtualKeyCode::E => Some(Input::RotateLeft),
            VirtualKeyCode::F => Some(Input::Flip),
            VirtualKeyCode::Up => Some(Input::Up),
            VirtualKeyCode::Down => Some(Input::Down),
            VirtualKeyCode::Left => Some(Input::Left),
            VirtualKeyCode::Right => Some(Input::Right),
            VirtualKeyCode::Return => Some(Input::Enter),
            VirtualKeyCode::Escape => Some(Input::Cancel),
            VirtualKeyCode::Key0 => Some(Input::GiveUp),
            _ => None,
        },
    }
}
