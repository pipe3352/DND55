//! A small ready-made campaign used by the "Quick Demo" button and by integration tests:
//! a lit guard room connected to a dark crypt.

use crate::command::{Command, DmCommand};
use crate::model::Principal;
use crate::session::Session;
use dnd_world::RoomId;
use dnd_world::map::{AmbientLight, LayerKind};

pub fn demo_campaign() -> Session {
    let mut s = Session::new_campaign("The Sunken Crypt");
    let entrance = s.state.party_room;
    let dm = |s: &mut Session, c: DmCommand| {
        s.apply(Principal::Dm, Command::Dm(c))
            .expect("demo command")
    };

    // Room 1: guard room, 16x12, stone floor, walls around, torches.
    s.state.graph.nodes.get_mut(&entrance).expect("room").name = "Guard Room".into();
    s.state
        .graph
        .nodes
        .get_mut(&entrance)
        .expect("room")
        .description = "A dusty guard room. A heavy door leads down into darkness.".into();
    build_room(&mut s, entrance, 16, 12);
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: entrance,
            asset: "torch".into(),
            cell: (1, 1),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: entrance,
            asset: "torch".into(),
            cell: (14, 1),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: entrance,
            asset: "table".into(),
            cell: (6, 5),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: entrance,
            asset: "barrel".into(),
            cell: (13, 9),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: entrance,
            asset: "door".into(),
            cell: (15, 6),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::SetEntryPoint {
            room: entrance,
            cell: (3, 6),
        },
    );

    // Room 2: the crypt — dark, so vision and light matter.
    dm(
        &mut s,
        DmCommand::CreateRoom {
            name: "Sunken Crypt".into(),
            width: 20,
            height: 14,
            ambient: AmbientLight::Darkness,
        },
    );
    let crypt = s.state.dm_view_room;
    build_room(&mut s, crypt, 20, 14);
    dm(
        &mut s,
        DmCommand::PaintTiles {
            room: crypt,
            layer: LayerKind::FloorDecoration,
            cells: vec![(8, 6), (9, 6), (8, 7), (9, 7)],
            asset: Some("rubble".into()),
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: crypt,
            asset: "altar".into(),
            cell: (16, 6),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: crypt,
            asset: "brazier".into(),
            cell: (15, 3),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: crypt,
            asset: "pillar".into(),
            cell: (6, 4),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: crypt,
            asset: "pillar".into(),
            cell: (6, 9),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room: crypt,
            asset: "chest".into(),
            cell: (17, 11),
            hidden: true,
        },
    );
    dm(
        &mut s,
        DmCommand::SetEntryPoint {
            room: crypt,
            cell: (2, 7),
        },
    );
    dm(
        &mut s,
        DmCommand::SpawnMonster {
            template: "skeleton".into(),
            room: crypt,
            cell: (14, 7),
            hidden: false,
        },
    );

    dm(
        &mut s,
        DmCommand::ConnectRooms {
            from: entrance,
            to: crypt,
            one_way: false,
        },
    );
    s.state.dm_view_room = entrance;
    s.state
        .graph
        .nodes
        .get_mut(&crypt)
        .expect("room")
        .editor_pos = (220.0, 60.0);
    s
}

fn build_room(s: &mut Session, room: RoomId, w: i32, h: i32) {
    let dm = |s: &mut Session, c: DmCommand| {
        s.apply(Principal::Dm, Command::Dm(c))
            .expect("demo command")
    };
    dm(
        s,
        DmCommand::FillRect {
            room,
            layer: LayerKind::Terrain,
            a: (0, 0),
            b: (w - 1, h - 1),
            asset: Some("stone_floor".into()),
        },
    );
    let mut walls = Vec::new();
    for x in 0..w {
        walls.push((x, 0));
        walls.push((x, h - 1));
    }
    for y in 1..h - 1 {
        walls.push((0, y));
        walls.push((w - 1, y));
    }
    dm(
        s,
        DmCommand::PaintTiles {
            room,
            layer: LayerKind::Walls,
            cells: walls,
            asset: Some("stone_wall".into()),
        },
    );
}
