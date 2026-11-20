//! Locks, keys, picking/forcing/breaking objects, inspection; DM journal, quests and room notes.

use dnd_rules::character::CharacterSheet;
use dnd_rules::items::ItemStack;
use dnd_rules::rng::ScriptedRng;
use dnd_session::command::{Command, DmCommand, ObjectAction};
use dnd_session::model::{JournalEntry, Objective, Principal, Quest, QuestStatus};
use dnd_session::scripts::{ScriptAction, ScriptedEvent, Trigger};
use dnd_session::{CommandError, Session};
use dnd_world::map::{LayerKind, Lock};
use dnd_world::{EntityId, ObjectId, RoomId};

const DM: Principal = Principal::Dm;

fn dm(s: &mut Session, c: DmCommand) {
    s.apply(DM, Command::Dm(c)).expect("dm command");
}

fn setup() -> (Session, RoomId, Principal, EntityId, ObjectId, ObjectId) {
    let mut s = Session::new_campaign("Locks");
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::FillRect {
            room,
            layer: LayerKind::Terrain,
            a: (0, 0),
            b: (9, 9),
            asset: Some("stone_floor".into()),
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room,
            asset: "heavy_door".into(),
            cell: (3, 2),
            hidden: false,
        },
    );
    dm(
        &mut s,
        DmCommand::PlaceObject {
            rotation: 0,
            room,
            asset: "chest".into(),
            cell: (1, 2),
            hidden: false,
        },
    );
    let ids: Vec<ObjectId> = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .map(|o| o.id)
        .collect();
    let (door, chest) = (ids[0], ids[1]);
    for (id, key) in [(door, Some("iron_key")), (chest, None)] {
        let mut o = s.state.rooms[&room].map.object(id).unwrap().clone();
        o.lock = Some(Lock {
            locked: true,
            dc: 12,
            key: key.map(String::from),
        });
        o.break_dc = Some(15);
        o.description = "Old oak bound in iron.".into();
        dm(
            &mut s,
            DmCommand::UpdateObject {
                room,
                object: Box::new(o),
            },
        );
    }
    let p = s.add_player("Alice");
    let who = Principal::Player(p);
    s.apply(
        who,
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
            cell: (2, 2),
        },
    );
    (s, room, who, alice, door, chest)
}

fn obj(s: &Session, room: RoomId, id: ObjectId) -> dnd_world::map::MapObject {
    s.state.rooms[&room].map.object(id).unwrap().clone()
}

#[test]
fn locked_objects_need_a_key_a_check_or_force() {
    let (mut s, room, who, alice, door, chest) = setup();
    // Locked: can't open; the player's map hides the DC and the key.
    assert!(
        matches!(s.apply(who, Command::Interact { entity: alice, object: door }), Err(CommandError::Rules(m)) if m.contains("locked"))
    );
    let pv = s.build_view(who, None);
    let _ = pv;
    let pmap = s.state.rooms[&room].map.player_view();
    let l = pmap.object(door).unwrap().lock.clone().unwrap();
    assert!(l.locked && l.dc == 0 && l.key.is_none());
    // No key → refused; with the key → unlocked, then it opens and stops blocking.
    assert!(
        s.apply(
            who,
            Command::ObjectAction {
                entity: alice,
                object: door,
                action: ObjectAction::UseKey
            }
        )
        .is_err()
    );
    let sheet = s
        .state
        .creatures
        .get_mut(&alice)
        .unwrap()
        .sheet_mut()
        .unwrap();
    sheet.inventory.push(ItemStack {
        id: "iron_key".into(),
        name: "Iron Key".into(),
        kind: dnd_rules::items::ItemKind::Gear,
        quantity: 1,
        value_cp: 0,
        description: String::new(),
    });
    s.apply(
        who,
        Command::ObjectAction {
            entity: alice,
            object: door,
            action: ObjectAction::UseKey,
        },
    )
    .unwrap();
    assert!(!obj(&s, room, door).is_locked());
    s.apply(
        who,
        Command::Interact {
            entity: alice,
            object: door,
        },
    )
    .unwrap();
    assert!(!s.state.rooms[&room].map.flags((3, 2)).blocks_movement);
    // Pick the chest: d20 = 1 fails, d20 = 20 succeeds.
    s.set_rng(Box::new(ScriptedRng::new([1])));
    s.apply(
        who,
        Command::ObjectAction {
            entity: alice,
            object: chest,
            action: ObjectAction::PickLock,
        },
    )
    .unwrap();
    assert!(obj(&s, room, chest).is_locked());
    s.set_rng(Box::new(ScriptedRng::new([20])));
    s.apply(
        who,
        Command::ObjectAction {
            entity: alice,
            object: chest,
            action: ObjectAction::ForceLock,
        },
    )
    .unwrap();
    assert!(!obj(&s, room, chest).is_locked());
}

#[test]
fn breaking_inspecting_scripts_and_persistence() {
    let (mut s, room, who, alice, door, chest) = setup();
    s.set_rng(Box::new(ScriptedRng::new([20])));
    s.apply(
        who,
        Command::ObjectAction {
            entity: alice,
            object: door,
            action: ObjectAction::Break,
        },
    )
    .unwrap();
    let d = obj(&s, room, door);
    assert!(d.destroyed && !d.is_locked());
    assert!(
        !s.state.rooms[&room].map.flags((3, 2)).blocks_movement
            && !s.state.rooms[&room].map.flags((3, 2)).blocks_sight
    );
    // Inspect: free, private to the player (and DM).
    let before = s.state.log.len();
    s.apply(
        who,
        Command::ObjectAction {
            entity: alice,
            object: chest,
            action: ObjectAction::Inspect,
        },
    )
    .unwrap();
    let e = &s.state.log[before];
    assert!(e.text.contains("Old oak") && e.text.contains("locked"));
    // A lever-style script can unlock the chest (WHEN interact → THEN unlock).
    dm(
        &mut s,
        DmCommand::SaveScript {
            script: Box::new(ScriptedEvent {
                id: 0,
                name: "Unlock".into(),
                enabled: true,
                once: true,
                fired: false,
                trigger: Trigger::Manual,
                actions: vec![ScriptAction::SetObjectLocked {
                    room,
                    object: chest,
                    locked: false,
                }],
            }),
        },
    );
    let sid = s.state.scripts[0].id;
    dm(
        &mut s,
        DmCommand::FireScript {
            id: sid,
            actor: None,
        },
    );
    assert!(!obj(&s, room, chest).is_locked());
    // Object state survives save/load.
    let json = serde_json::to_string(&s.state).unwrap();
    let back: dnd_session::GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s.state);
}

#[test]
fn journal_quests_and_room_notes_stay_hidden_until_shared() {
    let (mut s, room, who, _, _, _) = setup();
    dm(
        &mut s,
        DmCommand::SaveJournalEntry {
            entry: JournalEntry {
                title: "Secret".into(),
                body: "The duke is a vampire".into(),
                ..Default::default()
            },
        },
    );
    dm(
        &mut s,
        DmCommand::SaveJournalEntry {
            entry: JournalEntry {
                title: "Rumours".into(),
                body: "Wolves in the woods".into(),
                shared: true,
                ..Default::default()
            },
        },
    );
    dm(
        &mut s,
        DmCommand::SetRoomNotes {
            room,
            notes: "Three skeletons behind the north wall".into(),
        },
    );
    let q = Quest {
        title: "The Lost Relic".into(),
        status: QuestStatus::Undiscovered,
        objectives: vec![
            Objective {
                text: "Find the crypt".into(),
                ..Default::default()
            },
            Objective {
                text: "Betray the priest".into(),
                hidden: true,
                ..Default::default()
            },
        ],
        dm_notes: "The relic is fake".into(),
        ..Default::default()
    };
    dm(&mut s, DmCommand::SaveQuest { quest: q });
    let qid = s.state.quests[0].id;
    // Players: only shared entries, no undiscovered quests, no room notes.
    let v = s.build_view(who, None);
    assert_eq!(v.journal.len(), 1);
    assert_eq!(v.journal[0].title, "Rumours");
    assert!(v.quests.is_empty());
    assert!(
        v.graph
            .nodes
            .get(&room)
            .is_none_or(|n| n.dm_notes.is_empty())
    );
    assert!(
        !s.build_view(DM, None).graph.nodes[&room]
            .dm_notes
            .is_empty()
    );
    assert_eq!(s.build_view(DM, None).journal.len(), 2);
    // Reveal the quest, then a script ticks an objective.
    let mut q = s.state.quests[0].clone();
    q.status = QuestStatus::Active;
    dm(&mut s, DmCommand::SaveQuest { quest: q });
    assert!(
        s.state
            .log
            .iter()
            .any(|l| l.text.contains("New quest: The Lost Relic"))
    );
    dm(
        &mut s,
        DmCommand::SaveScript {
            script: Box::new(ScriptedEvent {
                id: 0,
                name: "Crypt found".into(),
                enabled: true,
                once: true,
                fired: false,
                trigger: Trigger::Manual,
                actions: vec![dnd_session::scripts::ScriptAction::UpdateQuest {
                    quest: qid,
                    status: None,
                    objective: Some((0, true)),
                }],
            }),
        },
    );
    let sid = s.state.scripts.last().unwrap().id;
    dm(
        &mut s,
        DmCommand::FireScript {
            id: sid,
            actor: None,
        },
    );
    let v = s.build_view(who, None);
    assert_eq!(v.quests.len(), 1);
    let pq = &v.quests[0];
    assert_eq!(pq.objectives.len(), 1, "hidden objective stays hidden");
    assert!(pq.objectives[0].done && pq.dm_notes.is_empty());
    // Players can't edit any of it.
    for c in [
        DmCommand::SaveJournalEntry {
            entry: JournalEntry {
                title: "x".into(),
                ..Default::default()
            },
        },
        DmCommand::SetRoomNotes {
            room,
            notes: "x".into(),
        },
        DmCommand::DeleteQuest { id: qid },
    ] {
        assert!(matches!(
            s.apply(who, Command::Dm(c)),
            Err(CommandError::NotAuthorized)
        ));
    }
}
