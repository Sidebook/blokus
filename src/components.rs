use crate::Mode;
use serde::{Serialize, Deserialize};
use specs::saveload::{Marker, ConvertSaveload};
use specs::error::NoError;
use super::Map;
use rltk::{Point, RGB};
use specs::prelude::*;
use specs_derive::*;
use std::cmp::min;
use crate::entity_vec::EntityVec;


#[derive(Component)]
pub struct SyncOnline;

#[derive(Component, ConvertSaveload, Clone)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub orig_x: i32,
    pub orig_y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Position {
            x: x,
            y: y,
            orig_x: x,
            orig_y: y,
        }
    }

    pub fn translate(&mut self, delta_x: i32, delta_y: i32) {
        self.x += delta_x;
        self.y += delta_y;
    }

    pub fn translate_within(&mut self, delta_x: i32, delta_y: i32, map: &Map) {
        let m = 2;
        let nx = self.x + delta_x;
        let ny = self.y + delta_y;
        if nx >= map.x - m
            && nx < map.x + map.width as i32 + m
            && ny >= map.y - m
            && ny < map.y + map.height as i32 + m
        {
            self.translate(delta_x, delta_y);
        }
    }

    pub fn to_point(&self) -> Point {
        return Point::new(self.x, self.y);
    }

    pub fn reset(&mut self) {
        self.x = self.orig_x;
        self.y = self.orig_y;
    }
}

#[derive(Component, ConvertSaveload, Clone)]
pub struct Polynomio {
    pub coods: Vec<Point>,
    pub orig_coods: Vec<Point>,
    pub color: RGB,
    pub fixed: bool,
    pub bg: bool,
}

impl Polynomio {
    pub fn new(coods: Vec<Point>, color: RGB, bg: bool) -> Self {
        Polynomio {
            coods: coods.clone(),
            orig_coods: coods,
            color,
            fixed: false,
            bg,
        }
    }

    pub fn rotate(&mut self, right: bool) {
        let s = if right { -1 } else { 1 };
        self.transform((0, s), (-s, 0));
    }

    pub fn flip(&mut self) {
        self.transform((-1, 0), (0, 1));
    }

    pub fn reset(&mut self) {
        self.coods = self.orig_coods.clone();
    }

    fn transform(&mut self, row1: (i32, i32), row2: (i32, i32)) {
        for cood in &mut self.coods {
            let tx = cood.x;
            let ty = cood.y;

            cood.x = row1.0 * tx + row1.1 * ty;
            cood.y = row2.0 * tx + row2.1 * ty;
        }
    }

    pub fn upper_left(&self) -> Point {
        let mut p = self.coods[0];
        for cood in &self.coods {
            p.x = min(p.x, cood.x);
            p.y = min(p.y, cood.y);
        }
        return p;
    }
}

#[derive(Component, ConvertSaveload, Clone)]
pub struct Player {
    pub id: i32,
    pub polynomios: EntityVec<Entity>,
    pub select: usize,
    pub fixed: Vec<bool>,
    pub color: RGB,
    pub end: bool,
    pub rank: i32,
    pub remaining_tiles: i32,
}

impl Player {
    pub fn new(id: i32, polynomios: Vec<Entity>, color: RGB) -> Self {
        let n = polynomios.len();
        return Player {
            id: id,
            polynomios: EntityVec::from_vec(polynomios),
            select: 0,
            fixed: vec![false; n],
            color: color,
            end: false,
            rank: 0,
            remaining_tiles: 0,
        };
    }
}

#[derive(Component)]
pub struct Rect {
    pub w: i32,
    pub h: i32,
}

#[derive(Component, ConvertSaveload, Clone)]
pub struct SerializeHelper {
    pub map: Map,
    pub active_player_id: usize,
    pub mode: Mode,
    // pub players: EntityVec<Entity>,
}