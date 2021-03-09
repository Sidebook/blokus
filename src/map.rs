use super::{Player, Polynomio};
use rltk::{Point, RGB};
use std::collections::HashMap;

pub const EMPTY: i32 = -1;
pub const WALL: i32 = -2;

const LINE_NEIGHBORS: [(i32, i32); 4] = [(-1, 0), (0, -1), (1, 0), (0, 1)];
const EDGE_NEIGHBORS: [(i32, i32); 4] = [(-1, -1), (1, -1), (1, 1), (-1, 1)];

pub struct Map {
    pub map: Vec<i32>,
    pub x: i32,
    pub y: i32,
    pub width: usize,
    pub height: usize,
    pub colors: HashMap<i32, RGB>,
    pub starts: HashMap<i32, Point>,
}

impl Map {
    pub fn new(x: i32, y: i32, width: usize, height: usize) -> Self {
        let mut map = vec![EMPTY; width * height];
        for i in 0..width {
            map[i] = WALL;
            map[i + width * (height - 1)] = WALL;
        }
        for i in 0..height {
            map[i * width] = WALL;
            map[i * width + width - 1] = WALL;
        }

        Map {
            map: map,
            x: x,
            y: y,
            width: width,
            height: height,
            colors: HashMap::new(),
            starts: HashMap::new(),
        }
    }

    pub fn bind(&mut self, player: &Player, start_x: i32, start_y: i32) {
        self.colors.insert(player.id, player.color);
        self.starts.insert(player.id, Point::new(start_x, start_y));
    }

    pub fn bind_left_top(&mut self, player: &Player) {
        self.bind(player, 1, 1);
    }

    pub fn bind_right_top(&mut self, player: &Player) {
        self.bind(player, self.width as i32 - 2, 1);
    }

    pub fn bind_left_bottom(&mut self, player: &Player) {
        self.bind(player, 1, self.height as i32 - 2);
    }

    pub fn bind_right_bottom(&mut self, player: &Player) {
        self.bind(player, self.width as i32 - 2, self.height as i32 - 2);
    }

    pub fn get(&self, p: Point) -> i32 {
        self.map[self.point_idx(p)]
    }

    pub fn touch_with_line(&self, p: Point, player_id: i32) -> bool {
        for n in LINE_NEIGHBORS.iter() {
            let np = Point::new(n.0, n.1) + p;
            if self.point_isin(np) && self.get(np) == player_id {
                return true;
            }
        }
        return false;
    }

    pub fn touch_with_edge(&self, p: Point, player_id: i32) -> bool {
        for n in EDGE_NEIGHBORS.iter() {
            let np = Point::new(n.0, n.1) + p;
            if self.point_isin(np) && self.get(np) == player_id {
                return true;
            }
        }
        return false;
    }

    pub fn try_put(&mut self, position: Point, polynomio: &Polynomio, player_id: i32) -> bool {
        let mut no_touch_with_line = true;
        let mut touch_with_edge = false;
        let mut include_start_position = false;

        for cood in &polynomio.coods {
            let p = *cood + position;
            if !self.point_isin(p) || self.get(p) != EMPTY {
                return false;
            }
            no_touch_with_line &= !self.touch_with_line(p, player_id);
            touch_with_edge |= self.touch_with_edge(p, player_id);
            include_start_position |= self.starts[&player_id] == p;
        }

        if !no_touch_with_line || (!touch_with_edge && !include_start_position) {
            return false;
        }

        for cood in &polynomio.coods {
            let p = *cood + position;
            let idx = self.point_idx(p);
            self.map[idx] = player_id;
        }
        true
    }

    pub fn try_remove(&mut self, position: Point, polynomio: &Polynomio, player_id: i32) -> bool {
        for cood in &polynomio.coods {
            let p = *cood + position;
            if !self.point_isin(p) || self.get(p) != player_id {
                return false;
            }
        }

        for cood in &polynomio.coods {
            let p = *cood + position;
            let idx = self.point_idx(p);
            self.map[idx] = EMPTY;
        }

        true
    }

    pub fn xy_idx(&self, x: i32, y: i32) -> usize {
        (x + y * self.width as i32) as usize
    }

    pub fn point_idx(&self, p: Point) -> usize {
        self.xy_idx(p.x, p.y)
    }

    pub fn xy_isin(&self, x: i32, y: i32) -> bool {
        x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32
    }

    pub fn point_isin(&self, p: Point) -> bool {
        self.xy_isin(p.x, p.y)
    }

    pub fn idx_point(&self, idx: usize) -> Point {
        Point::new(
            idx as i32 % self.width as i32,
            idx as i32 / self.width as i32,
        )
    }
}
