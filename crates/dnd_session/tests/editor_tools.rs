//! Map editor tools: resize with anchor, universal eraser, rotation/facing, deep undo, permissions.

use dnd_rules::character::CharacterSheet;
use dnd_session::command::{Command, DmCommand, EraseMode};
use dnd_session::model::Principal;
use dnd_session::{CommandError, Session};
use dnd_world::map::{Anchor, LayerKind};
use dnd_world::{EntityId, RoomId};

const DM: Principal = Principal::Dm;

fn dm(s: &mut Session, c: DmCommand) {
    s.apply(DM, Command::Dm(c)).expect("dm command");
}

/// 10×8 room: grass floor, a barrel at (8,6), a chest at (2,2), a wall tile, a light, a goblin
/// and a player character.
fn room() -> (Session, RoomId, EntityId, EntityId, Principal) {
    let mut s = Session::new_campaign("Edit");
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::ResizeMap {
            room,
            width: 10,
            height: 8,
            anchor: Anchor::TopLeft,
            confirm_loss: true,
        },
    );
    dm(
        &mut s,
        DmCommand::FillRect {
            room,
            layer: LayerKind::Terrain,
            a: (0, 0),
            b: (9, 7),
            asset: Some("grass".into()),
        },
    );
    dm(
        &mut s,
        DmCommand::FillRect {
            room,
            layer: LayerKind::Walls,
            a: (5, 0),
            b: (5, 0),
            asset: Some("stone_wall".into()),
        },
    );
    for (asset, cell) in [("barrel", (8, 6)), ("chest", (2, 2))] {
        dm(
            &mut s,
            DmCommand::PlaceObject {
                rotation: 0,
                room,
                asset: asset.into(),
                cell,
                hidden: false,
            },
        );
    }
    dm(
        &mut s,
        DmCommand::AddLight {
            room,
            cell: (4, 4),
            bright_ft: 20,
            dim_ft: 20,
        },
    );
    dm(
        &mut s,
        DmCommand::SpawnMonster {
            template: "goblin_warrior".into(),
            room,
            cell: (3, 3),
            hidden: false,
        },
    );
    let gob = *s.state.creatures.keys().max().unwrap();
    let p = s.add_player("Alice");
    s.apply(
        Principal::Player(p),
        Command::SubmitCharacter {
            sheet: Box::new(CharacterSheet::example_fighter("Alice")),
            portrait_png: None,
        },
    )
    .unwrap();
    let alice = s.state.players[&p].character.unwrap();
    dm(
        &mut s,
        DmCommand::Teleport {
            entity: alice,
            cell: (1, 1),
        },
    );
    (s, room, gob, alice, Principal::Player(p))
}

#[test]
fn resizing_grows_with_anchor_moves_everything_and_undoes() {
    let (mut s, room, gob, alice, _) = room();
    let before = s.state.clone();
    dm(
        &mut s,
        DmCommand::ResizeMap {
            room,
            width: 14,
            height: 10,
            anchor: Anchor::Center,
            confirm_loss: false,
        },
    );
    let rs = &s.state.rooms[&room];
    assert_eq!((rs.map.width, rs.map.height), (14, 10));
    // Centre anchor: offset (2,1).
    assert!(
        rs.map
            .objects
            .iter()
            .any(|o| o.asset_id == "barrel" && (o.x, o.y) == (10, 7))
    );
    assert_eq!(rs.extra_lights[0].cell, (6, 5));
    assert_eq!(s.state.creatures[&gob].pos, (5, 4));
    assert_eq!(s.state.creatures[&alice].pos, (3, 2));
    assert_eq!(rs.explored.len(), 140);
    // Undo restores map, lights and creature positions; redo re-applies.
    dm(&mut s, DmCommand::Undo { room });
    assert_eq!(s.state.rooms[&room].map, before.rooms[&room].map);
    assert_eq!(s.state.creatures[&gob].pos, (3, 3));
    assert_eq!(s.state.creatures[&alice].pos, (1, 1));
    dm(&mut s, DmCommand::Redo { room });
    assert_eq!(s.state.rooms[&room].map.width, 14);
    // Save/load keeps the resized map.
    let json = serde_json::to_string(&s.state).unwrap();
    let back: dnd_session::GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s.state);
}

#[test]
fn shrinking_warns_and_never_deletes_creatures() {
    let (mut s, room, gob, _, _) = room();
    // Cutting the barrel's corner needs confirmation; nothing changes until confirmed.
    let snap = s.state.clone();
    let e = s.apply(
        DM,
        Command::Dm(DmCommand::ResizeMap {
            room,
            width: 8,
            height: 6,
            anchor: Anchor::TopLeft,
            confirm_loss: false,
        }),
    );
    assert!(matches!(e, Err(CommandError::Rules(m)) if m.contains("confirm")));
    assert_eq!(s.state, snap);
    // A creature outside the new area blocks the resize even when confirmed.
    let e = s.apply(
        DM,
        Command::Dm(DmCommand::ResizeMap {
            room,
            width: 3,
            height: 3,
            anchor: Anchor::TopLeft,
            confirm_loss: true,
        }),
    );
    assert!(matches!(e, Err(CommandError::Rules(m)) if m.contains("Goblin")));
    dm(
        &mut s,
        DmCommand::ResizeMap {
            room,
            width: 8,
            height: 6,
            anchor: Anchor::TopLeft,
            confirm_loss: true,
        },
    );
    let rs = &s.state.rooms[&room];
    assert!(!rs.map.objects.iter().any(|o| o.asset_id == "barrel"));
    assert!(s.state.creatures.contains_key(&gob));
}

#[test]
fn universal_eraser_modes_respect_locks_and_protection() {
    let (mut s, room, gob, alice, _) = room();
    let all: Vec<(i32, i32)> = (0..10).flat_map(|x| (0..8).map(move |y| (x, y))).collect();
    // Category: only furniture objects (chest is furniture? use the catalog's category).
    let chest_cat = dnd_world::catalog::Catalog::builtin()
        .get("chest")
        .unwrap()
        .category;
    dm(
        &mut s,
        DmCommand::EraseArea {
            room,
            cells: vec![(2, 2), (8, 6)],
            mode: EraseMode::Objects(Some(chest_cat)),
            protect: false,
        },
    );
    let objs: Vec<String> = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .map(|o| o.asset_id.clone())
        .collect();
    assert!(!objs.contains(&"chest".to_string()));
    let barrel_cat = dnd_world::catalog::Catalog::builtin()
        .get("barrel")
        .unwrap()
        .category;
    assert_eq!(
        objs.contains(&"barrel".to_string()),
        barrel_cat != chest_cat
    );
    // Selected layer: walls only.
    dm(
        &mut s,
        DmCommand::EraseArea {
            room,
            cells: vec![(5, 0)],
            mode: EraseMode::Layer(LayerKind::Walls),
            protect: false,
        },
    );
    assert!(
        s.state.rooms[&room]
            .map
            .tile(LayerKind::Walls, (5, 0))
            .is_none()
    );
    assert!(
        s.state.rooms[&room]
            .map
            .tile(LayerKind::Terrain, (5, 0))
            .is_some()
    );
    // Locked terrain survives "erase everything"; creatures go (never the player character).
    dm(
        &mut s,
        DmCommand::SetLayer {
            room,
            layer: LayerKind::Terrain,
            visible: true,
            locked: true,
        },
    );
    dm(
        &mut s,
        DmCommand::EraseArea {
            room,
            cells: all.clone(),
            mode: EraseMode::All,
            protect: false,
        },
    );
    let rs = &s.state.rooms[&room];
    assert!(rs.map.tile(LayerKind::Terrain, (0, 0)).is_some());
    assert!(rs.map.objects.is_empty() && rs.extra_lights.is_empty());
    assert!(!s.state.creatures.contains_key(&gob));
    assert!(s.state.creatures.contains_key(&alice));
    // Undo brings the goblin and the objects back.
    dm(&mut s, DmCommand::Undo { room });
    assert!(s.state.creatures.contains_key(&gob));
    assert!(!s.state.rooms[&room].map.objects.is_empty());
    // Protection keeps doors/interactive/containers; unlocked terrain is reset to empty.
    dm(
        &mut s,
        DmCommand::SetLayer {
            room,
            layer: LayerKind::Terrain,
            visible: true,
            locked: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room,
            asset: "door".into(),
            cell: (7, 1),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::EraseArea {
            room,
            cells: all,
            mode: EraseMode::All,
            protect: true,
        },
    );
    let rs = &s.state.rooms[&room];
    assert!(rs.map.objects.iter().all(|o| o.asset_id == "door"));
    assert!(rs.map.tile(LayerKind::Terrain, (0, 0)).is_none());
}

#[test]
fn rotation_before_placement_facing_and_permissions() {
    let (mut s, room, gob, _, alice_p) = room();
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 1,
            room,
            asset: "table".into(),
            cell: (6, 3),
            hidden: false,
        },
    );
    let t = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .find(|o| o.asset_id == "table")
        .unwrap()
        .clone();
    assert_eq!(t.rotation, 1);
    // A 2×1 table rotated 90° covers 1×2 cells.
    let cells: Vec<_> = t.footprint().collect();
    assert_eq!(cells, vec![(6, 3), (6, 4)]);
    dm(
        &mut s,
        DmCommand::SetFacing {
            entity: gob,
            facing: 2,
        },
    );
    assert_eq!(s.state.creatures[&gob].facing, 2);
    let v = s.build_view(DM, None);
    assert_eq!(v.creatures.iter().find(|c| c.id == gob).unwrap().facing, 2);
    let json = serde_json::to_string(&s.state).unwrap();
    let back: dnd_session::GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(back.creatures[&gob].facing, 2);
    // Players can't use the editor tools.
    for c in [
        DmCommand::ResizeMap {
            room,
            width: 20,
            height: 20,
            anchor: Anchor::Center,
            confirm_loss: true,
        },
        DmCommand::EraseArea {
            room,
            cells: vec![(1, 1)],
            mode: EraseMode::All,
            protect: false,
        },
        DmCommand::SetFacing {
            entity: gob,
            facing: 1,
        },
    ] {
        let snap = s.state.clone();
        assert!(matches!(
            s.apply(alice_p, Command::Dm(c)),
            Err(CommandError::NotAuthorized)
        ));
        assert_eq!(s.state, snap);
    }
}
