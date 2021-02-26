use super::{Map, Player, Polynomio, Position, Rect, EMPTY, WALL};
use rltk::{Rltk, RGB};

pub fn draw_map(ctx: &mut Rltk, map: &Map) {
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
            map.colors[key] * 0.8,
            RGB::named(rltk::WHITE) * 0.9,
            rltk::to_cp437('X'),
        );
    }
}

pub fn draw_polynomio(ctx: &mut Rltk, pos: &Position, polynomio: &Polynomio) {
    for cood in &polynomio.coods {
        ctx.set(
            pos.x + cood.x,
            pos.y + cood.y,
            polynomio.color,
            polynomio.color * 0.8,
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

pub fn draw_ui(ctx: &mut Rltk, position: &Position, player: &Player, active_player_id: usize) {
    let player_str = if player.end {
        format!["Player #{} (Finished)", &(player.id + 1)]
    } else if player.id as usize == active_player_id {
        format!["Player #{} <= Your turn", &(player.id + 1)]
    } else {
        format!["Player #{}", &(player.id + 1)]
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
