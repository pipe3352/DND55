//! End-to-end session tests: authority, hidden information, combat, reactions, room transitions.

use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_rules::d20::RollMode;
use dnd_rules::rng::{GameRng, ScriptedRng};
use dnd_session::command::{Command, DmCommand};
use dnd_session::demo::demo_campaign;
use dnd_session::event::{Event, FloatKind};
use dnd_session::model::{Faction, Principal};
use dnd_session::view::HpView;
use dnd_session::{CommandError, Session};
use dnd_world::map::{AmbientLight, LayerKind};
use dnd_world::{EntityId, RoomId};

const DM: Principal = Principal::Dm;

fn dm(s: &mut Session, c: DmCommand) -> Vec<dnd_session::event::Routed> {
    s.apply(DM, Command::Dm(c)).expect("dm command")
}

fn join_with_fighter(s: &mut Session, name: &str) -> (Principal, EntityId) {
    let p = s.add_player(name);
    let who = Principal::Player(p);
    s.apply(
        who,
        Command::SubmitCharacter {
            sheet: Box::new(CharacterSheet::example_fighter(name)),
            portrait_png: None,
        },
    )
    .unwrap();
    let id = s.state.players[&p].character.unwrap();
    (who, id)
}

/// Lit open arena with no monsters.
fn arena() -> Session {
    let mut s = Session::new_campaign("Arena");
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::FillRect {
            room,
            layer: LayerKind::Terrain,
            a: (0, 0),
            b: (19, 14),
            asset: Some("stone_floor".into()),
        },
    );
    s.set_rng(Box::new(GameRng::seeded(3)));
    s
}

fn spawn(s: &mut Session, template: &str, cell: (i32, i32), hidden: bool) -> EntityId {
    let room = s.state.party_room;
    dm(
        s,
        DmCommand::SpawnMonster {
            template: template.into(),
            room,
            cell,
            hidden,
        },
    );
    *s.state.creatures.keys().max().unwrap()
}

#[test]
fn players_cannot_use_dm_commands_or_move_others() {
    let mut s = arena();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    let (bob, _) = join_with_fighter(&mut s, "Bob");
    let room = s.state.party_room;
    let r = s.apply(
        alice,
        Command::Dm(DmCommand::SpawnMonster {
            template: "skeleton".into(),
            room,
            cell: (5, 5),
            hidden: false,
        }),
    );
    assert_eq!(r, Err(CommandError::NotAuthorized));
    assert_eq!(
        s.apply(
            bob,
            Command::Move {
                entity: a_id,
                to: (8, 8)
            }
        ),
        Err(CommandError::NotAuthorized)
    );
    assert!(
        s.apply(
            alice,
            Command::Move {
                entity: a_id,
                to: (8, 8)
            }
        )
        .is_ok()
    );
    assert_eq!(s.state.creatures[&a_id].pos, (8, 8));
    // An unknown player id is rejected outright.
    let ghost = Principal::Player(dnd_session::PlayerId(999));
    assert_eq!(
        s.apply(ghost, Command::Chat { text: "hi".into() }),
        Err(CommandError::NotAuthorized)
    );
}

#[test]
fn invalid_characters_are_rejected_by_the_host() {
    let mut s = arena();
    let p = Principal::Player(s.add_player("Cheater"));
    let mut sheet = CharacterSheet::example_fighter("Cheater");
    sheet.base_scores = dnd_rules::AbilityScores::uniform(18);
    let r = s.apply(
        p,
        Command::SubmitCharacter {
            sheet: Box::new(sheet),
            portrait_png: None,
        },
    );
    assert!(matches!(r, Err(CommandError::Invalid(_))));
    assert!(s.state.creatures.is_empty());
    // Garbage portrait data is rejected too.
    let r = s.apply(
        p,
        Command::SubmitCharacter {
            sheet: Box::new(CharacterSheet::example_fighter("Ok")),
            portrait_png: Some(b"nope".to_vec()),
        },
    );
    assert!(matches!(r, Err(CommandError::Invalid(_))));
}

#[test]
fn hidden_monsters_and_objects_never_reach_players() {
    let mut s = arena();
    let (alice, _) = join_with_fighter(&mut s, "Alice");
    let hidden = spawn(&mut s, "skeleton", (10, 10), true);
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::PlaceObject {
            room,
            asset: "chest".into(),
            cell: (12, 12),
            hidden: true,
        },
    );

    let v = s.build_view(alice, None);
    assert!(v.creatures.iter().all(|c| c.id != hidden));
    let map = v.room.as_ref().unwrap().map.as_ref().unwrap();
    assert!(map.objects.is_empty());
    let json = serde_json::to_string(&v).unwrap();
    assert!(
        !json.contains("Skeleton"),
        "no trace of the hidden monster in the player's payload"
    );

    // The DM sees everything.
    let v = s.build_view(DM, None);
    assert!(v.creatures.iter().any(|c| c.id == hidden && c.hidden));

    // Revealing makes it visible; players see only a health status, not exact HP or stats.
    dm(
        &mut s,
        DmCommand::SetCreatureHidden {
            entity: hidden,
            hidden: false,
        },
    );
    let v = s.build_view(alice, None);
    let sk = v.creatures.iter().find(|c| c.id == hidden).unwrap();
    assert!(matches!(sk.hp, HpView::Status(_)));
    assert!(sk.attacks.is_empty() && sk.armor_class.is_none());
}

#[test]
fn darkness_hides_monsters_until_light_or_darkvision() {
    let mut s = Session::new_campaign("Dark");
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::SetAmbient {
            room,
            ambient: AmbientLight::Darkness,
        },
    );
    let p = s.add_player("Hal");
    let who = Principal::Player(p);
    let mut sheet = CharacterSheet::example_fighter("Hal");
    sheet.species = dnd_rules::content::SpeciesId::Human; // no Darkvision
    sheet.size = dnd_rules::content::Size::Medium;
    sheet.species_skill = Some(dnd_rules::Skill::Insight);
    s.apply(
        who,
        Command::SubmitCharacter {
            sheet: Box::new(sheet),
            portrait_png: None,
        },
    )
    .unwrap();
    let me = s.state.players[&p].character.unwrap();
    s.apply(
        who,
        Command::Move {
            entity: me,
            to: (2, 2),
        },
    )
    .unwrap();
    let sk = spawn(&mut s, "skeleton", (6, 2), false);
    assert!(
        !s.build_view(who, None).creatures.iter().any(|c| c.id == sk),
        "pitch dark: human can't see it"
    );
    let vis = s.build_view(who, None).room.unwrap().visible.unwrap();
    assert!(vis.iter().all(|v| *v == 0));
    // A light spell / torch placed by the DM lights it up.
    dm(
        &mut s,
        DmCommand::AddLight {
            room,
            cell: (4, 2),
            bright_ft: 20,
            dim_ft: 20,
        },
    );
    assert!(s.build_view(who, None).creatures.iter().any(|c| c.id == sk));
}

#[test]
fn darkvision_sees_in_darkness_in_gray() {
    let mut s = Session::new_campaign("Dark");
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::SetAmbient {
            room,
            ambient: AmbientLight::Darkness,
        },
    );
    let (who, me) = join_with_fighter(&mut s, "Dwarfy"); // Dwarf: Darkvision 120 ft
    s.apply(
        who,
        Command::Move {
            entity: me,
            to: (2, 2),
        },
    )
    .unwrap();
    let sk = spawn(&mut s, "skeleton", (8, 2), false);
    let v = s.build_view(who, None);
    assert!(v.creatures.iter().any(|c| c.id == sk));
    let room_v = v.room.unwrap();
    assert_eq!(
        room_v.visible.unwrap()[(2 * room_v.width + 8) as usize],
        1,
        "gray (darkvision in darkness)"
    );
}

#[test]
fn full_combat_turn_attack_and_floating_damage() {
    let mut s = arena();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (5, 5),
        },
    )
    .unwrap();
    let goblin = spawn(&mut s, "goblin_warrior", (6, 5), false);

    // Initiative: Alice d20=15 (+1 Dex) = 16; goblin d20=3 (+2) = 5.
    s.set_rng(Box::new(ScriptedRng::new([15, 3])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    let cb = s.state.combat.as_ref().unwrap();
    assert_eq!(cb.order[0].entity, a_id);
    assert_eq!(cb.turn.movement_left_ft, 30);

    // Not the goblin's turn: the DM can't attack with it yet.
    assert!(matches!(
        s.apply(
            DM,
            Command::Attack {
                attacker: goblin,
                target: a_id,
                attack: 0
            }
        ),
        Err(CommandError::Rules(_))
    ));

    // Longsword (+5) vs AC 15: d20=12 -> 17 hits; 1d8+3 with d8=6 -> 9 damage. Goblin has 10 HP.
    s.set_rng(Box::new(ScriptedRng::new([12, 6])));
    let ev = s
        .apply(
            alice,
            Command::Attack {
                attacker: a_id,
                target: goblin,
                attack: 0,
            },
        )
        .unwrap();
    assert_eq!(s.state.creatures[&goblin].hp.current, 1);
    assert!(ev.iter().any(|r| matches!(&r.event, Event::Floating { entity, text, kind: FloatKind::Damage } if *entity == goblin && text == "-9")));
    // Sap mastery: the goblin has Disadvantage on its next attack.
    assert_eq!(s.state.creatures[&goblin].effects.sapped_by, Some(a_id));
    // Only one Attack action per turn.
    assert!(matches!(
        s.apply(
            alice,
            Command::Attack {
                attacker: a_id,
                target: goblin,
                attack: 0
            }
        ),
        Err(CommandError::Rules(_))
    ));

    s.apply(alice, Command::EndTurn).unwrap();
    assert_eq!(s.state.combat.as_ref().unwrap().turn.entity, goblin);
    // Goblin scimitar with Disadvantage from Sap: d20s 18 and 4 -> 4+4 = 8 vs AC 18 -> miss.
    s.set_rng(Box::new(ScriptedRng::new([18, 4])));
    let ev = s
        .apply(
            DM,
            Command::Attack {
                attacker: goblin,
                target: a_id,
                attack: 0,
            },
        )
        .unwrap();
    assert!(ev.iter().any(|r| matches!(
        &r.event,
        Event::Floating {
            kind: FloatKind::Miss,
            ..
        }
    )));
    assert_eq!(
        s.state.creatures[&goblin].effects.sapped_by, None,
        "Sap consumed"
    );
    s.apply(DM, Command::EndTurn).unwrap();
    assert_eq!(s.state.combat.as_ref().unwrap().round, 2);

    // Natural 20 crit kills it: monsters die at 0 HP and leave the initiative.
    s.set_rng(Box::new(ScriptedRng::new([20, 1, 1])));
    s.apply(
        alice,
        Command::Attack {
            attacker: a_id,
            target: goblin,
            attack: 0,
        },
    )
    .unwrap();
    assert!(s.state.creatures[&goblin].hp.dead);
    assert!(
        s.state
            .combat
            .as_ref()
            .unwrap()
            .order
            .iter()
            .all(|e| e.entity != goblin)
    );
}

#[test]
fn movement_is_limited_by_speed_and_dash() {
    let mut s = arena();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (1, 1),
        },
    )
    .unwrap();
    spawn(&mut s, "skeleton", (18, 13), false);
    s.set_rng(Box::new(ScriptedRng::new([20, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    assert!(
        matches!(
            s.apply(
                alice,
                Command::Move {
                    entity: a_id,
                    to: (8, 1)
                }
            ),
            Err(CommandError::Rules(_))
        ),
        "35 ft > 30 ft"
    );
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (7, 7),
        },
    )
    .unwrap(); // 6 diagonal squares = 30 ft
    assert_eq!(s.state.combat.as_ref().unwrap().turn.movement_left_ft, 0);
    s.apply(alice, Command::Dash { entity: a_id }).unwrap();
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (13, 7),
        },
    )
    .unwrap();
    assert!(
        s.apply(alice, Command::Dodge { entity: a_id }).is_err(),
        "action already used"
    );
}

#[test]
fn opportunity_attack_interrupts_movement_and_waits_for_a_decision() {
    let mut s = arena();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (5, 5),
        },
    )
    .unwrap();
    let sk = spawn(&mut s, "skeleton", (6, 5), false);
    s.set_rng(Box::new(ScriptedRng::new([20, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });

    // Alice walks away to (1,5): leaving reach at the first step triggers the prompt.
    let ev = s
        .apply(
            alice,
            Command::Move {
                entity: a_id,
                to: (1, 5),
            },
        )
        .unwrap();
    assert!(
        ev.iter()
            .any(|r| matches!(r.event, Event::ReactionPrompt { reactor, .. } if reactor == sk))
    );
    assert_eq!(
        s.state.creatures[&a_id].pos,
        (5, 5),
        "attack happens before she leaves reach"
    );
    // Nobody else can act until the DM decides; players can't decide for the skeleton.
    assert!(s.apply(alice, Command::EndTurn).is_err());
    assert_eq!(
        s.apply(
            alice,
            Command::ReactionDecision {
                accept: true,
                attack: 0
            }
        ),
        Err(CommandError::NotAuthorized)
    );
    // DM accepts: shortsword +5, d20=10 -> 15 vs AC 18 miss. Movement then resumes automatically.
    s.set_rng(Box::new(ScriptedRng::new([10])));
    s.apply(
        DM,
        Command::ReactionDecision {
            accept: true,
            attack: 0,
        },
    )
    .unwrap();
    assert_eq!(s.state.creatures[&a_id].pos, (1, 5));
    assert!(
        s.state
            .combat
            .as_ref()
            .unwrap()
            .reactions_used
            .contains(&sk)
    );
}

#[test]
fn disengage_prevents_opportunity_attacks() {
    let mut s = arena();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (5, 5),
        },
    )
    .unwrap();
    spawn(&mut s, "skeleton", (6, 5), false);
    s.set_rng(Box::new(ScriptedRng::new([20, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    s.apply(alice, Command::Disengage { entity: a_id }).unwrap();
    let ev = s
        .apply(
            alice,
            Command::Move {
                entity: a_id,
                to: (1, 5),
            },
        )
        .unwrap();
    assert!(
        !ev.iter()
            .any(|r| matches!(r.event, Event::ReactionPrompt { .. }))
    );
    assert_eq!(s.state.creatures[&a_id].pos, (1, 5));
}

#[test]
fn surprise_gives_disadvantage_on_initiative() {
    let mut s = arena();
    let (_alice, a_id) = join_with_fighter(&mut s, "Alice");
    spawn(&mut s, "skeleton", (10, 10), false);
    // Alice surprised: rolls 18 and 2 -> keeps 2.
    s.set_rng(Box::new(ScriptedRng::new([18, 2, 10])));
    dm(
        &mut s,
        DmCommand::StartCombat {
            surprised: vec![a_id],
        },
    );
    let e = s
        .state
        .combat
        .as_ref()
        .unwrap()
        .order
        .iter()
        .find(|e| e.entity == a_id)
        .unwrap()
        .clone();
    assert_eq!(e.roll.mode, RollMode::Disadvantage);
    assert_eq!(e.roll.natural, 2);
}

#[test]
fn dropping_to_zero_then_death_saves() {
    let mut s = arena();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (5, 5),
        },
    )
    .unwrap();
    let sk = spawn(&mut s, "skeleton", (6, 5), false);
    s.set_rng(Box::new(ScriptedRng::new([1, 20]))); // skeleton goes first
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    assert_eq!(s.state.combat.as_ref().unwrap().turn.entity, sk);
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: a_id,
            delta: -13,
        },
    );
    let c = &s.state.creatures[&a_id];
    assert_eq!(c.hp.current, 0);
    assert!(c.conditions.has(Condition::Unconscious) && c.conditions.has(Condition::Prone));
    // Attacks within 5 ft vs Unconscious: Advantage and auto-crit. d20s 7,12 -> 12+5=17 < 18 AC... use 15,14 -> 15+5=20 hit -> crit = 2 failures.
    s.set_rng(Box::new(ScriptedRng::new([15, 14, 3, 3])));
    s.apply(
        DM,
        Command::Attack {
            attacker: sk,
            target: a_id,
            attack: 0,
        },
    )
    .unwrap();
    assert_eq!(s.state.creatures[&a_id].hp.death_saves.failures, 2);
    // Alice's turn starts: Death Saving Throw d20=12 -> success.
    s.set_rng(Box::new(ScriptedRng::new([12])));
    s.apply(DM, Command::EndTurn).unwrap();
    assert_eq!(s.state.creatures[&a_id].hp.death_saves.successes, 1);
    // Healing brings her back (and she stays Prone).
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: a_id,
            delta: 5,
        },
    );
    let c = &s.state.creatures[&a_id];
    assert_eq!(c.hp.current, 5);
    assert!(!c.conditions.has(Condition::Unconscious) && c.conditions.has(Condition::Prone));
}

#[test]
fn only_dm_moves_party_between_connected_rooms_and_state_persists() {
    let mut s = demo_campaign();
    s.set_rng(Box::new(GameRng::seeded(9)));
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    let start = s.state.party_room;
    let crypt: RoomId = *s.state.rooms.keys().find(|r| **r != start).unwrap();

    // Walking into the door cell does not change rooms; players can't trigger transitions.
    let r = s.apply(
        alice,
        Command::Dm(DmCommand::TransitionParty {
            to: crypt,
            override_rules: false,
        }),
    );
    assert_eq!(r, Err(CommandError::NotAuthorized));

    // The crypt is not in the players' graph until visited.
    assert!(!s.build_view(alice, None).graph.nodes.contains_key(&crypt));

    let ev = dm(
        &mut s,
        DmCommand::TransitionParty {
            to: crypt,
            override_rules: false,
        },
    );
    assert!(
        ev.iter()
            .any(|r| matches!(r.event, Event::RoomChanged { room, .. } if room == crypt))
    );
    assert_eq!(s.state.creatures[&a_id].room, crypt);
    assert_eq!(s.state.party_room, crypt);
    assert!(s.build_view(alice, None).graph.nodes.contains_key(&crypt));

    // Kill the skeleton, go back and forth: it stays dead (room state persists).
    let sk = s
        .state
        .creatures
        .values()
        .find(|c| c.faction == Faction::Hostile)
        .unwrap()
        .id;
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: sk,
            delta: -100,
        },
    );
    dm(
        &mut s,
        DmCommand::TransitionParty {
            to: start,
            override_rules: false,
        },
    );
    dm(
        &mut s,
        DmCommand::TransitionParty {
            to: crypt,
            override_rules: false,
        },
    );
    assert!(s.state.creatures[&sk].hp.dead);

    // Unconnected rooms need a DM override.
    dm(
        &mut s,
        DmCommand::CreateRoom {
            name: "Island".into(),
            width: 5,
            height: 5,
            ambient: AmbientLight::Bright,
        },
    );
    let island = s.state.dm_view_room;
    assert!(
        s.apply(
            DM,
            Command::Dm(DmCommand::TransitionParty {
                to: island,
                override_rules: false
            })
        )
        .is_err()
    );
    assert!(
        s.apply(
            DM,
            Command::Dm(DmCommand::TransitionParty {
                to: island,
                override_rules: true
            })
        )
        .is_ok()
    );
}

#[test]
fn map_edits_support_undo_and_redo_and_failed_commands_change_nothing() {
    let mut s = arena();
    let room = s.state.party_room;
    let before = s.state.rooms[&room].map.clone();
    dm(
        &mut s,
        DmCommand::PaintTiles {
            room,
            layer: LayerKind::Walls,
            cells: vec![(3, 3), (4, 3)],
            asset: Some("stone_wall".into()),
        },
    );
    assert!(s.state.rooms[&room].map.flags((3, 3)).blocks_movement);
    dm(&mut s, DmCommand::Undo { room });
    assert_eq!(s.state.rooms[&room].map, before);
    dm(&mut s, DmCommand::Redo { room });
    assert!(s.state.rooms[&room].map.flags((4, 3)).blocks_movement);

    let snapshot = s.state.clone();
    let r = s.apply(
        DM,
        Command::Dm(DmCommand::PaintTiles {
            room,
            layer: LayerKind::Walls,
            cells: vec![(1, 1)],
            asset: Some("no_such_asset".into()),
        }),
    );
    assert!(r.is_err());
    assert_eq!(s.state, snapshot);
}

#[test]
fn private_rolls_only_go_to_dm_and_roller() {
    let mut s = arena();
    let (alice, _) = join_with_fighter(&mut s, "Alice");
    let (bob, _) = join_with_fighter(&mut s, "Bob");
    let ev = s
        .apply(
            alice,
            Command::RollDice {
                expr: "1d20+5".into(),
                mode: RollMode::Advantage,
                private: true,
                label: "Stealth".into(),
            },
        )
        .unwrap();
    let vis_bob = s.visible_creature_ids(bob);
    let vis_alice = s.visible_creature_ids(alice);
    assert!(s.events_for(bob, &ev, &vis_bob).is_empty());
    assert!(!s.events_for(alice, &ev, &vis_alice).is_empty());
    assert!(!s.events_for(DM, &ev, &Default::default()).is_empty());
    assert!(
        s.apply(
            alice,
            Command::RollDice {
                expr: "2d6".into(),
                mode: RollMode::Advantage,
                private: false,
                label: String::new()
            }
        )
        .is_err()
    );
    assert!(
        s.apply(
            alice,
            Command::RollDice {
                expr: "999d6".into(),
                mode: RollMode::Normal,
                private: false,
                label: String::new()
            }
        )
        .is_err()
    );
}

#[test]
fn doors_can_be_opened_by_adjacent_characters() {
    let mut s = demo_campaign();
    let (alice, a_id) = join_with_fighter(&mut s, "Alice");
    let room = s.state.party_room;
    let door = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .find(|o| o.asset_id == "door")
        .unwrap()
        .id;
    assert!(
        s.apply(
            alice,
            Command::Interact {
                entity: a_id,
                object: door
            }
        )
        .is_err(),
        "too far"
    );
    s.apply(
        alice,
        Command::Move {
            entity: a_id,
            to: (14, 6),
        },
    )
    .unwrap();
    s.apply(
        alice,
        Command::Interact {
            entity: a_id,
            object: door,
        },
    )
    .unwrap();
    assert!(s.state.rooms[&room].map.object(door).unwrap().open);
}

#[test]
fn map_is_only_resent_when_revision_changes() {
    let mut s = arena();
    let (alice, _) = join_with_fighter(&mut s, "Alice");
    let v = s.build_view(alice, None);
    let r = v.room.unwrap();
    let known = Some((r.id, r.map_rev));
    assert!(s.build_view(alice, known).room.unwrap().map.is_none());
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::PlaceObject {
            room,
            asset: "barrel".into(),
            cell: (2, 2),
            hidden: false,
        },
    );
    assert!(s.build_view(alice, known).room.unwrap().map.is_some());
}

#[test]
fn game_state_roundtrips_through_json() {
    let mut s = demo_campaign();
    join_with_fighter(&mut s, "Alice");
    let json = serde_json::to_string(&s.state).unwrap();
    let back: dnd_session::GameState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, s.state);
}
