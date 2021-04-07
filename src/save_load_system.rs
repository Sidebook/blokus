use crate::Map;
use crate::Mode;
use crate::SerializeHelper;
use crate::SyncOnline;
use crate::{Player, Polynomio, Position};
use specs::error::NoError;
use specs::prelude::*;
use specs::saveload::MarkedBuilder;
use specs::saveload::SimpleMarker;
use specs::saveload::SimpleMarkerAllocator;
use specs::saveload::{DeserializeComponents, SerializeComponents};

macro_rules! serialize_individually {
    ($ecs:expr, $ser:expr, $data:expr, $( $type:ty),*) => {
        $(
        SerializeComponents::<NoError, SimpleMarker<SyncOnline>>::serialize(
            &( $ecs.read_storage::<$type>(), ),
            &$data.0,
            &$data.1,
            &mut $ser,
        )
        .unwrap();
        )*
    };
}

pub fn dump_game(ecs: &mut World) -> String {
    // Create helper
    let (mapcopy, active_player_id, mode) = {
        // let players = (*ecs.fetch_mut::<Vec<Entity>>()).clone();
        (
            (*ecs.fetch_mut::<Map>()).clone(),
            *ecs.fetch_mut::<usize>(),
            *ecs.fetch_mut::<Mode>(),
        )
    };

    let savehelper = ecs
        .create_entity()
        .with(SerializeHelper {
            map: mapcopy,
            active_player_id: active_player_id,
            mode: mode,
        })
        .marked::<SimpleMarker<SyncOnline>>()
        .build();

    // Actually serialize
    let data = {
        let data = (
            ecs.entities(),
            ecs.read_storage::<SimpleMarker<SyncOnline>>(),
        );

        let serialized_bytes: Vec<u8> = Vec::new();
        let mut serializer = serde_json::Serializer::new(serialized_bytes);
        serialize_individually!(
            ecs,
            serializer,
            data,
            Position,
            Polynomio,
            Player,
            SerializeHelper
        );
        String::from_utf8(serializer.into_inner()).expect("Failed to serialize: broken byte array.")
    };

    // Clean up
    ecs.delete_entity(savehelper).expect("Crash on cleanup");

    data
}

macro_rules! deserialize_individually {
    ($ecs:expr, $de:expr, $data:expr, $( $type:ty),*) => {
        $(
        DeserializeComponents::<NoError, _>::deserialize(
            &mut ( &mut $ecs.write_storage::<$type>(), ),
            &mut $data.0, // entities
            &mut $data.1, // marker
            &mut $data.2, // allocater
            &mut $de,
        )
        .unwrap();
        )*
    };
}

pub fn load_game(ecs: &mut World, data: &str) {
    {
        // Delete everything
        let mut to_delete = Vec::new();
        for e in ecs.entities().join() {
            to_delete.push(e);
        }
        for del in to_delete.iter() {
            ecs.delete_entity(*del).expect("Deletion failed");
        }
    }

    let mut de = serde_json::Deserializer::from_str(data);

    {
        let mut d = (
            &mut ecs.entities(),
            &mut ecs.write_storage::<SimpleMarker<SyncOnline>>(),
            &mut ecs.write_resource::<SimpleMarkerAllocator<SyncOnline>>(),
        );

        deserialize_individually!(ecs, de, d, Position, Polynomio, Player, SerializeHelper);
    }

    let mut deleteme: Option<Entity> = None;
    {
        let entities = ecs.entities();
        let helper = ecs.read_storage::<SerializeHelper>();
        let players = ecs.read_storage::<Player>();

        for (e, h) in (&entities, &helper).join() {
            let mut worldmap = ecs.write_resource::<super::map::Map>();
            *worldmap = h.map.clone();
            let mut active_player_id = ecs.write_resource::<usize>();
            *active_player_id = h.active_player_id;
            let mut mode = ecs.write_resource::<Mode>();
            *mode = h.mode;
            // let mut players = ecs.write_resource::<Vec<Entity>>();
            // *players = h.players.0.clone();
            deleteme = Some(e);
        }

        let mut player_entities = ecs.fetch_mut::<Vec<Entity>>();
        let mut player_entity_vec = (&entities, &players)
            .join()
            .collect::<Vec<(Entity, &Player)>>();
        player_entity_vec.sort_by(|a, b| (a.1.id).cmp(&b.1.id));
        *player_entities = player_entity_vec
            .iter()
            .map(|(e, _)| *e)
            .collect::<Vec<Entity>>();

        // for (e,h) in (&entities, &helper).join() {
        //     let mut worldmap = ecs.write_resource::<super::map::Map>();
        //     *worldmap = h.map.clone();
        //     worldmap.tile_content = vec![Vec::new(); super::map::MAPCOUNT];
        //     // deleteme = Some(e);
        // }
        // for (e,_p,pos) in (&entities, &player, &position).join() {
        //     let mut ppos = ecs.write_resource::<rltk::Point>();
        //     *ppos = rltk::Point::new(pos.x, pos.y);
        //     let mut player_resource = ecs.write_resource::<Entity>();
        //     *player_resource = e;
        // }
    }
    ecs.delete_entity(deleteme.unwrap())
        .expect("Unable to delete helper");
}
