use super::{Map, Player, Polynomio, Position, State};
use rltk::Point;
use specs::prelude::*;
use specs::Entity;

pub trait Event {
    fn undo(&mut self, gs: &mut State);
    fn should_chain_next(&self) -> bool;
}

pub struct PutEvent {
    pub player_entity: Entity,
    pub polynomio_id: usize,
}

impl Event for PutEvent {
    fn undo(&mut self, gs: &mut State) {
        let mut map = gs.ecs.fetch_mut::<Map>();
        let mut player_store = gs.ecs.write_storage::<Player>();
        let player = player_store.get_mut(self.player_entity).unwrap();
        let polynomio_entity = player.polynomios[self.polynomio_id];
        let mut position_store = gs.ecs.write_storage::<Position>();
        let mut polynomio_store = gs.ecs.write_storage::<Polynomio>();
        let polynomio = polynomio_store.get_mut(polynomio_entity).unwrap();
        let position = position_store.get_mut(polynomio_entity).unwrap();

        let remove_pos = Point::new(position.x - map.x, position.y - map.y);
        if !map.try_remove(remove_pos, polynomio, player.id) {
            panic!("failed to undo");
        }
        player.fixed[self.polynomio_id] = false;
        player.select = self.polynomio_id;
        position.reset();
        polynomio.reset();
    }
    fn should_chain_next(&self) -> bool {
        false
    }
}

pub struct TurnChangeEvent {
    pub from: usize,
}

impl Event for TurnChangeEvent {
    fn undo(&mut self, gs: &mut State) {
        let mut active_player_id = gs.ecs.fetch_mut::<usize>();
        *active_player_id = self.from;
    }

    fn should_chain_next(&self) -> bool {
        true
    }
}

pub struct GiveUpEvent {
    pub player_entity: Entity,
}

impl Event for GiveUpEvent {
    fn undo(&mut self, gs: &mut State) {
        let mut player_store = gs.ecs.write_storage::<Player>();
        let mut player = player_store.get_mut(self.player_entity).unwrap();
        player.end = false;
    }

    fn should_chain_next(&self) -> bool {
        false
    }
}
