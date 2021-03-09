use super::{Entity, Player, Polynomio};
use specs::ReadExpect;
use specs::ReadStorage;
use specs::System;
use specs::WriteStorage;

pub struct PolynomioIndexingSystem {}

impl<'a> System<'a> for PolynomioIndexingSystem {
    type SystemData = (
        ReadExpect<'a, Vec<Entity>>,
        ReadStorage<'a, Player>,
        WriteStorage<'a, Polynomio>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (player_entities, players_store, mut polynomios_store) = data;
        let players: Vec<&Player> = player_entities
            .iter()
            .map(|pe| players_store.get(*pe).unwrap())
            .collect();

        for &player in players.iter() {
            for (&fixed, &polynomio_entity) in player.fixed.iter().zip(player.polynomios.iter()) {
                let polynomio = polynomios_store.get_mut(polynomio_entity).unwrap();
                polynomio.fixed = fixed;
            }
        }
    }
}
