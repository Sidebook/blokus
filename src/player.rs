use super::{Map, Mode, Player, Polynomio, Position, State, Input};
use rltk::{Point, Rltk, VirtualKeyCode};
use specs::Entity;
use specs::WorldExt;

pub fn player_input(gs: &mut State, ctx: &mut Rltk) -> Mode {
    let active_player_id = *gs.ecs.read_resource::<usize>();
    let player_id = if gs.use_local_input {active_player_id as i32} else {gs.my_player_id};
    match ctx.key {
        None => {}
        Some(key) => match key {
            VirtualKeyCode::R => {
                gs.push_input(player_id, Input::RotateRight);
            }
            VirtualKeyCode::E => {
                gs.push_input(player_id, Input::RotateLeft);
            }
            VirtualKeyCode::F => {
                gs.push_input(player_id, Input::Flip);
            }
            VirtualKeyCode::Up => {
                gs.push_input(player_id, Input::Up);
            }
            VirtualKeyCode::Down => {
                gs.push_input(player_id, Input::Down);
            }
            VirtualKeyCode::Left => {
                gs.push_input(player_id, Input::Left);
            }
            VirtualKeyCode::Right => {
                gs.push_input(player_id, Input::Right);
            }
            VirtualKeyCode::Return => {
                gs.push_input(player_id, Input::Enter);
            }
            VirtualKeyCode::Escape => {
                gs.push_input(player_id, Input::Cancel);
            }
            VirtualKeyCode::Key0 => {
                gs.push_input(player_id, Input::GiveUp);
            }
            _ => {}
        },
    }

    let mode = *gs.ecs.fetch::<Mode>();
    match mode {
        Mode::Initialize => return Mode::Select,
        Mode::Select => player_input_select(gs),
        Mode::Put => player_input_put(gs),
        Mode::Finish => return Mode::Finish,
    }
}

fn player_input_select(gs: &mut State) -> Mode {
    let mut ended = false;
    {
        let active_player_id = *gs.ecs.read_resource::<usize>();
        let input = gs.pop_for(active_player_id as i32);
        let map = gs.ecs.write_resource::<Map>();
        let mut players = gs.ecs.write_storage::<Player>();
        
        let player = players
            .get_mut(gs.ecs.fetch::<Vec<Entity>>()[active_player_id])
            .unwrap();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut active_position = positions.get_mut(player.polynomios[player.select]).unwrap();

        match input {
            None => {}
            Some(key) => match key {
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
            },
        }
    }
    if ended {
        gs.next_player();
    }
    Mode::Select
}

fn player_input_put(gs: &mut State) -> Mode {
    let mut next_player = false;
    let _ended = false;
    {
        let active_player_id = *gs.ecs.read_resource::<usize>();
        let input = gs.pop_for(active_player_id as i32);
        let mut map = gs.ecs.write_resource::<Map>();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut polynomios = gs.ecs.write_storage::<Polynomio>();
        let mut players = gs.ecs.write_storage::<Player>();
        
        let player = players
            .get_mut(gs.ecs.fetch::<Vec<Entity>>()[active_player_id])
            .unwrap();
        let active_position = positions.get_mut(player.polynomios[player.select]).unwrap();
        let active_polynomio = polynomios
            .get_mut(player.polynomios[player.select])
            .unwrap();

        match input {
            None => {}
            Some(key) => match key {
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
            },
        }
    }
    if next_player {
        gs.next_player();
        return Mode::Select;
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
