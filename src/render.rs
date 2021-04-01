use crate::PlayerSlotManager;
use std::sync::Mutex;
use actix_web::web::Data;
use crate::Mode;
use specs::prelude::*;

use super::{Map, Player, Polynomio, Position, Rect, EMPTY, WALL};
use rltk::{Rltk, RGB};


pub fn render(ecs: &World, ctx: &mut Rltk, slot_manager: Option<Data<Mutex<PlayerSlotManager>>>) {
    ctx.cls();

    let mode = *ecs.fetch::<Mode>();

    let dialogs = match mode {
        Mode::Initialize => vec![],
        Mode::Select => {
            vec!["Left/Right: Select a piece to put  Enter: Put  Num0: Give up".to_string()]
        }
        Mode::Put => vec![
            "Left/Right/Up/Down: Move a piece  Enter: Put  Num0: Give up".to_string(),
            "R: Rotate right  E: Rotate left  F: Flip  Esc: Cancel".to_string(),
        ],
        Mode::Finish => vec![format!["Player ??? won!"]],
    };
    for (i, dialog) in dialogs.iter().enumerate() {
        ctx.print(5, i * 2 + 60, dialog);
    }

    draw_map(ecs, ctx);
    draw_uis(&ecs, ctx, slot_manager);
    
    draw_polynomios(&ecs, ctx, mode, true);
    draw_polynomios(&ecs, ctx, mode, false);

    let players = ecs.read_storage::<Player>();
    let active_player_id = ecs.read_resource::<usize>();

    let positions = ecs.read_storage::<Position>();
    let polynomios = ecs.read_storage::<Polynomio>();

    // let player = players
    //     .get(ecs.fetch::<Vec<Entity>>()[*active_player_id])
    //     .unwrap();

    let entities = ecs.entities();
    if mode != Mode::Finish {
        for (_, player) in (&entities, &players).join() {
            if player.id != *active_player_id as i32 {
                continue;
            }
            let active_position = positions.get(player.polynomios[player.select]).unwrap();
            let active_polynomio = polynomios.get(player.polynomios[player.select]).unwrap();

            draw_polynomio(ctx, active_position, active_polynomio, 1.);

            let upper_left = polynomios
                .get(player.polynomios[player.select])
                .unwrap()
                .upper_left();
            ctx.set(
                active_position.x + upper_left.x - 1,
                active_position.y + upper_left.y,
                player.color,
                RGB::named(rltk::BLACK),
                rltk::to_cp437('>'),
            );
        }
    }
}

pub fn draw_map(ecs: & World, ctx: &mut Rltk) {
    let map = ecs.read_resource::<Map>();

    for (idx, &tile) in map.map.iter().enumerate() {
        let p = map.idx_point(idx);
        if tile == EMPTY {
            ctx.set(
                p.x + map.x,
                p.y + map.y,
                RGB::named(rltk::WHITE) * 0.8,
                RGB::named(rltk::WHITE) * 0.9,
                rltk::to_cp437('■'),
            );
        } else if tile == WALL {
            ctx.set(
                p.x + map.x,
                p.y + map.y,
                RGB::named(rltk::WHITE) * 0.2,
                RGB::named(rltk::WHITE) * 0.6,
                rltk::to_cp437(' '),
            );
        }
    }

    for key in map.colors.keys() {
        ctx.set(
            map.starts[key].x + map.x,
            map.starts[key].y + map.y,
            map.colors[key],
            RGB::named(rltk::WHITE) * 0.9,
            rltk::to_cp437('■'),
        );
    }
}

pub fn draw_polynomios(
    ecs: & World,
    ctx: &mut Rltk,
    mode: Mode,
    bg: bool,
) {
    let positions = ecs.read_storage::<Position>();
    let polynomios = ecs.read_storage::<Polynomio>();
    for (pos, polynomio) in (&positions, &polynomios).join() {
        if polynomio.bg != bg {
            continue;
        }
        let alpha = if mode == Mode::Put && polynomio.fixed {
            0.7
        } else {
            1.0
        };
        draw_polynomio(ctx, pos, polynomio, alpha);
    }
}

pub fn draw_polynomio(ctx: &mut Rltk, pos: &Position, polynomio: &Polynomio, alpha: f32) {
    let color = polynomio.color * alpha;
    for cood in &polynomio.coods {
        ctx.set(
            pos.x + cood.x,
            pos.y + cood.y,
            color,
            color * 0.8,
            rltk::to_cp437('■'),
        );
    }
}

pub fn draw_rect(ctx: &mut Rltk, position: &Position, rect: &Rect) {
    for xi in position.x..position.x + rect.w {
        ctx.set(
            xi,
            position.y,
            RGB::named(rltk::WHITE),
            RGB::named(rltk::BLACK),
            rltk::to_cp437('-'),
        );
        ctx.set(
            xi,
            position.y + rect.h - 1,
            RGB::named(rltk::WHITE),
            RGB::named(rltk::BLACK),
            rltk::to_cp437('-'),
        );
    }
    for yi in position.y..position.y + rect.h {
        ctx.set(
            position.x,
            yi,
            RGB::named(rltk::WHITE),
            RGB::named(rltk::BLACK),
            rltk::to_cp437('|'),
        );
        ctx.set(
            position.x + rect.w - 1,
            yi,
            RGB::named(rltk::WHITE),
            RGB::named(rltk::BLACK),
            rltk::to_cp437('|'),
        );
    }
}


pub fn draw_uis(ecs: &World, ctx: &mut Rltk, slot_manager: Option<Data<Mutex<PlayerSlotManager>>>) {
    let positions = ecs.read_storage::<Position>();
    let players = ecs.read_storage::<Player>();
    let active_player_id = ecs.fetch::<usize>();

    for (pos, player) in (&positions, &players).join() {
        draw_ui(ctx, pos, player, *active_player_id, slot_manager.clone());
    }
}

pub fn draw_ui(ctx: &mut Rltk, position: &Position, player: &Player, active_player_id: usize, slot_manager: Option<Data<Mutex<PlayerSlotManager>>>) {

    let player_name = if let Some(slot_manager) = slot_manager {
        if let Some(slot) = slot_manager.lock().unwrap().get(player.id as usize) {
            format!("{}", slot.name.clone())
        } else {
            format!("Player #{} (Not connected)", player.id + 1)
        }
    } else {
        format!("Player #{}", player.id + 1)
    };

    let player_str = if player.end {
        format!["{} (Finished)", player_name]
    } else if player.id as usize == active_player_id {
        format!["{} <= Your turn", player_name]
    } else {
        format!["{}", player_name]
    };

    let stats = &format!["remaining: {} (#{})", player.remaining_tiles, player.rank];
    let dialog = &format!["{:<30}{:>33}", player_str, stats];

    ctx.print_color(
        position.x,
        position.y,
        player.color,
        RGB::named(rltk::BLACK),
        dialog,
    );
}
