use super::{
    GiveUpEvent, Input, Map, Mode, Player, Polynomio, Position, PutEvent, State, TurnChangeEvent,
};
use crate::{ClientState, UserInput};
use rltk::{Point, Rltk, VirtualKeyCode};
use specs::Entity;
use specs::WorldExt;

#[derive(Clone, PartialEq, Debug)]
pub enum InputResult {
    Updated {
        trigger: Option<UserInput>,
        newmode: Mode,
    },
    Noop,
}

use InputResult::*;

pub fn player_input(gs: &mut State, user_input: UserInput) -> InputResult {
    let mode = *gs.ecs.fetch::<Mode>();

    if user_input.input == Input::RequestBroadcast {
        return Updated {
            newmode: mode,
            trigger: Some(user_input),
        };
    }

    if user_input.input == Input::Undo && mode == Mode::Select {
        gs.undo();
        return Updated {
            newmode: mode,
            trigger: Some(user_input),
        };
    }

    let active_player_id = *gs.ecs.read_resource::<usize>() as i32;
    let result = match (user_input.player_id, mode) {
        (_, Mode::Initialize) => Updated {
            newmode: Mode::Select,
            trigger: None,
        },
        (pid, Mode::Select) if pid == active_player_id => player_input_select(gs, user_input),
        (pid, Mode::Put) if pid == active_player_id => player_input_put(gs, user_input),
        (_, _) => Noop,
    };

    if let Updated {
        newmode: _,
        trigger,
    } = result.clone()
    {
        if gs.is_finished() {
            return Updated {
                newmode: Mode::Finish,
                trigger,
            };
        }
    }
    result
}

fn player_input_select(gs: &mut State, user_input: UserInput) -> InputResult {
    let player_entity: Entity;
    let active_player_id: usize;
    let mut ended = false;
    let mut updated = false;
    let mut newmode = Mode::Select;
    {
        active_player_id = *gs.ecs.read_resource::<usize>();
        player_entity = gs.ecs.fetch::<Vec<Entity>>()[active_player_id];
        let map = gs.ecs.write_resource::<Map>();
        let mut players = gs.ecs.write_storage::<Player>();
        let player = players.get_mut(player_entity).unwrap();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut active_position = positions.get_mut(player.polynomios[player.select]).unwrap();

        match user_input.input {
            Input::Right => {
                updated = select_next(player, false);
            }
            Input::Left => {
                updated = select_next(player, true);
            }
            Input::Enter => {
                active_position.x = map.width as i32 / 2 + map.x;
                active_position.y = map.height as i32 / 2 + map.y;
                updated = true;
                newmode = Mode::Put
            }
            Input::GiveUp => {
                player.end = true;
                ended = true;
                updated = true;
            }
            _ => {}
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

    match updated {
        true => Updated {
            newmode,
            trigger: Some(user_input),
        },
        false => Noop,
    }
}

fn player_input_put(gs: &mut State, user_input: UserInput) -> InputResult {
    let mut next_player = false;
    let mut ended = false;
    let active_player_id: usize;
    let player_entity;
    let player_select;
    let player;
    let mut updated = false;
    let mut newmode = Mode::Put;
    {
        active_player_id = *gs.ecs.read_resource::<usize>();
        player_entity = gs.ecs.fetch::<Vec<Entity>>()[active_player_id];
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

        match user_input.input {
            Input::RotateRight => {
                active_polynomio.rotate(true);
                updated = true;
            }
            Input::RotateLeft => {
                active_polynomio.rotate(false);
                updated = true;
            }
            Input::Flip => {
                active_polynomio.flip();
                updated = true;
            }
            Input::Up => {
                active_position.translate_within(0, -1, &*map);
                updated = true;
            }
            Input::Down => {
                active_position.translate_within(0, 1, &*map);
                updated = true;
            }
            Input::Left => {
                active_position.translate_within(-1, 0, &*map);
                updated = true;
            }
            Input::Right => {
                active_position.translate_within(1, 0, &*map);
                updated = true;
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
                    updated = true;
                    newmode = Mode::Select;
                }
            }
            Input::Cancel => {
                active_position.reset();
                active_polynomio.reset();
                updated = true;
                newmode = Mode::Select;
            }
            _ => {}
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
    }

    match updated {
        true => Updated {
            newmode,
            trigger: Some(user_input),
        },
        false => Noop,
    }
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
            VirtualKeyCode::Z => Some(Input::Undo),
            _ => None,
        },
    }
}

pub fn player_input_client(gs: &mut ClientState, ctx: &mut Rltk) -> Mode {
    let mode = *gs.ecs.fetch::<Mode>();

    if gs.locked {
        return mode;
    }

    let active_player_id = *gs.ecs.read_resource::<usize>();
    if gs.player_id as usize != active_player_id {
        return mode;
    }

    let input = map_virtual_key_code(ctx.key);

    match input {
        Some(input) => match mode {
            Mode::Initialize => Mode::Select,
            Mode::Select => {
                let player_entity = gs.ecs.fetch::<Vec<Entity>>()[active_player_id];
                let map = gs.ecs.write_resource::<Map>();
                let mut players = gs.ecs.write_storage::<Player>();
                let player = players.get_mut(player_entity).unwrap();
                let mut positions = gs.ecs.write_storage::<Position>();
                let mut active_position =
                    positions.get_mut(player.polynomios[player.select]).unwrap();
                match input {
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
                    }
                    _ => {}
                }
                Mode::Select
            }
            Mode::Put => {
                let player_entity = gs.ecs.fetch::<Vec<Entity>>()[active_player_id];

                let mut map = gs.ecs.write_resource::<Map>();
                let mut positions = gs.ecs.write_storage::<Position>();
                let mut polynomios = gs.ecs.write_storage::<Polynomio>();
                let mut players = gs.ecs.write_storage::<Player>();
                let player = players.get_mut(player_entity).unwrap();
                let active_position = positions.get_mut(player.polynomios[player.select]).unwrap();
                let active_polynomio = polynomios
                    .get_mut(player.polynomios[player.select])
                    .unwrap();
                match input {
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
                        let put_to =
                            Point::new(active_position.x - map.x, active_position.y - map.y);
                        map.try_put(put_to, active_polynomio, active_player_id as i32);
                        gs.locked = true;
                    }
                    Input::Cancel => {
                        active_position.reset();
                        active_polynomio.reset();
                        return Mode::Select;
                    }
                    _ => {}
                }
                Mode::Put
            }
            Mode::Finish => Mode::Finish,
        },
        None => mode,
    }
}
