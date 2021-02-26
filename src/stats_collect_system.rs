use super::{Player, Polynomio};
use specs::Entity;
use specs::ReadExpect;
use specs::ReadStorage;
use specs::System;
use specs::WriteStorage;

pub struct StatsCollectSystem {
    pub winner: usize,
}

impl<'a> System<'a> for StatsCollectSystem {
    type SystemData = (
        ReadExpect<'a, Vec<Entity>>,
        WriteStorage<'a, Player>,
        ReadStorage<'a, Polynomio>,
    );
    fn run(&mut self, data: Self::SystemData) {
        let (player_entities, mut players_store, polynomios_store) = data;
        let players: Vec<&Player> = player_entities
            .iter()
            .map(|pe| players_store.get(*pe).unwrap())
            .collect();

        let mut totals: Vec<(i32, usize)> = Vec::new();
        for (i, player) in players.iter().enumerate() {
            let polynomios: Vec<&Polynomio> = player
                .polynomios
                .iter()
                .map(|pe| polynomios_store.get(*pe).unwrap())
                .collect();
            let mut total: i32 = 0;
            for (p, fixed) in polynomios.iter().zip(player.fixed.iter()) {
                if !fixed {
                    total += p.coods.len() as i32;
                }
            }
            totals.push((total, i));
        }

        totals.sort_by_key(|e| (e.0, -(e.1 as i32)));
        self.winner = totals[0].1;
        for (rank, (total, i)) in totals.iter().enumerate() {
            let p = player_entities.get(*i).unwrap();
            players_store.get_mut(*p).unwrap().remaining_tiles = *total;
            players_store.get_mut(*p).unwrap().rank = (rank + 1) as i32;
        }
    }
}
