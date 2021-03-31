use specs::saveload::SimpleMarkerAllocator;
use std::io::Cursor;
use crate::Polynomio;
use crate::Position;
use std::fs::File;
use crate::SyncOnline;
use specs::saveload::SimpleMarker;
use specs::error::NoError;
use specs::saveload::{SerializeComponents, DeserializeComponents};
use specs::prelude::*;

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

pub fn dump_game(ecs : &mut World) -> String {
    // Create helper
    // let mapcopy = ecs.get_mut::<super::map::Map>().unwrap().clone();
    // let savehelper = ecs
        // .create_entity()
        // .with(SerializationHelper{ map : mapcopy })
        // .marked::<SimpleMarker<SerializeMe>>()
        // .build();

    // Actually serialize
    // {
        let data = ( ecs.entities(), ecs.read_storage::<SimpleMarker<SyncOnline>>() );

        let serialized_bytes: Vec<u8> = Vec::new();
        // let writer = File::create("./savegame.json").unwrap();
        // serde_json::to_string(data);
        let mut serializer = serde_json::Serializer::new(serialized_bytes);
        serialize_individually!(ecs, serializer, data, Position, Polynomio);
        String::from_utf8(serializer.into_inner()).expect("Failed to serialize: broken byte array.")
        // println!("{}", std::str::from_utf8(&serializer.into_inner()).unwrap());
    // }

    // Clean up
    // ecs.delete_entity(savehelper).expect("Crash on cleanup");
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
        let mut d = (&mut ecs.entities(), &mut ecs.write_storage::<SimpleMarker<SyncOnline>>(), &mut ecs.write_resource::<SimpleMarkerAllocator<SyncOnline>>());

        deserialize_individually!(ecs, de, d, Position, Polynomio);
    }

    let mut deleteme : Option<Entity> = None;
    {
        let entities = ecs.entities();
        // let helper = ecs.read_storage::<SerializationHelper>();
        let position = ecs.read_storage::<Position>();
        let player = ecs.read_storage::<Polynomio>();
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
    // ecs.delete_entity(deleteme.unwrap()).expect("Unable to delete helper");
}