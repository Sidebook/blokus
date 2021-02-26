use super::{Map, Mode, Player, Polynomio, Position, State};
use rltk::{Point, Rltk, VirtualKeyCode};
use specs::Entity;
use specs::WorldExt;

pub fn player_input(gs: &mut State, ctx: &mut Rltk) -> Mode {
    let mode = *gs.ecs.fetch::<Mode>();
    match mode {
        Mode::Initialize => return Mode::Select,
        Mode::Select => player_input_select(gs, ctx),
        Mode::Put => player_input_put(gs, ctx),
        Mode::Finish => return Mode::Finish,
    }
}

fn player_input_select(gs: &mut State, ctx: &mut Rltk) -> Mode {
    let mut ended = false;
    {
        let map = gs.ecs.write_resource::<Map>();
        let mut players = gs.ecs.write_storage::<Player>();
        let active_player_id = gs.ecs.read_resource::<usize>();
        let player = players
            .get_mut(gs.ecs.fetch::<Vec<Entity>>()[*active_player_id])
            .unwrap();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut active_position = positions.get_mut(player.polynomios[player.select]).unwrap();

        match ctx.key {
            None => {}
            Some(key) => match key {
                VirtualKeyCode::Right => {
                    select_next(player, false);
                }
                VirtualKeyCode::Left => {
                    select_next(player, true);
                }
                VirtualKeyCode::Return => {
                    active_position.x = map.width as i32 / 2 + map.x;
                    active_position.y = map.height as i32 / 2 + map.y;
                    return Mode::Put;
                }
                VirtualKeyCode::Key0 => {
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

fn player_input_put(gs: &mut State, ctx: &mut Rltk) -> Mode {
    let mut next_player = false;
    let _ended = false;
    {
        let mut map = gs.ecs.write_resource::<Map>();
        let mut positions = gs.ecs.write_storage::<Position>();
        let mut polynomios = gs.ecs.write_storage::<Polynomio>();
        let mut players = gs.ecs.write_storage::<Player>();
        let active_player_id = gs.ecs.read_resource::<usize>();
        let player = players
            .get_mut(gs.ecs.fetch::<Vec<Entity>>()[*active_player_id])
            .unwrap();
        let active_position = positions.get_mut(player.polynomios[player.select]).unwrap();
        let active_polynomio = polynomios
            .get_mut(player.polynomios[player.select])
            .unwrap();

        match ctx.key {
            None => {}
            Some(key) => match key {
                VirtualKeyCode::R => {
                    active_polynomio.rotate(true);
                }
                VirtualKeyCode::E => {
                    active_polynomio.rotate(false);
                }
                VirtualKeyCode::F => {
                    active_polynomio.flip();
                }
                VirtualKeyCode::Up => {
                    active_position.translate_within(0, -1, &*map);
                }
                VirtualKeyCode::Down => {
                    active_position.translate_within(0, 1, &*map);
                }
                VirtualKeyCode::Left => {
                    active_position.translate_within(-1, 0, &*map);
                }
                VirtualKeyCode::Right => {
                    active_position.translate_within(1, 0, &*map);
                }
                VirtualKeyCode::Return => {
                    active_position.to_point();
                    let put_to = Point::new(active_position.x - map.x, active_position.y - map.y);

                    if map.try_put(put_to, active_polynomio, *active_player_id as i32) {
                        player.fixed[player.select] = true;
                        if !select_next(player, false) {
                            player.end = true;
                        }
                        next_player = true;
                    }
                }
                VirtualKeyCode::Escape => {
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
