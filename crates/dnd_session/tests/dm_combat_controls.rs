//! DM initiative and Hit Point controls: overrides, ordering, undo, sync, persistence, authority.

use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_rules::damage::DamageType;
use dnd_rules::rng::GameRng;
use dnd_session::command::{Command, DmCommand, HpOp};
use dnd_session::model::Principal;
use dnd_session::view::HpView;
use dnd_session::{CommandError, Session};
use dnd_world::EntityId;
use dnd_world::map::LayerKind;

const DM: Principal = Principal::Dm;

fn dm(s: &mut Session, c: DmCommand) {
    s.apply(DM, Command::Dm(c)).expect("dm command");
}

/// Arena with Alice (fighter), Bob (fighter) and two goblins, in combat.
fn battle() -> (Session, Principal, EntityId, EntityId, EntityId, EntityId) {
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
    s.set_rng(Box::new(GameRng::seeded(11)));
    let join = |s: &mut Session, name: &str| {
        let p = s.add_player(name);
        s.apply(
            Principal::Player(p),
            Command::SubmitCharacter {
                sheet: Box::new(CharacterSheet::example_fighter(name)),
                portrait_png: None,
            },
        )
        .unwrap();
        (Principal::Player(p), s.state.players[&p].character.unwrap())
    };
    let (alice_p, alice) = join(&mut s, "Alice");
    let (_, bob) = join(&mut s, "Bob");
    for (i, cell) in [(8, 5), (9, 5)].into_iter().enumerate() {
        dm(
            &mut s,
            DmCommand::SpawnMonster {
                template: "goblin_warrior".into(),
                room,
                cell,
                hidden: false,
            },
        );
        let _ = i;
    }
    let mut ids: Vec<EntityId> = s
        .state
        .creatures
        .keys()
        .copied()
        .filter(|id| *id != alice && *id != bob)
        .collect();
    ids.sort();
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    (s, alice_p, alice, bob, ids[0], ids[1])
}

fn order(s: &Session) -> Vec<(EntityId, i32)> {
    s.state
        .combat
        .as_ref()
        .unwrap()
        .order
        .iter()
        .map(|e| (e.entity, e.total))
        .collect()
}
fn active(s: &Session) -> EntityId {
    s.state.combat.as_ref().unwrap().turn.entity
}
fn hp(s: &Session, id: EntityId) -> (i32, i32, i32) {
    let h = &s.state.creatures[&id].hp;
    (h.current, h.max, h.temp)
}

#[test]
fn initiative_edits_reorder_and_keep_the_active_turn_and_round() {
    let (mut s, _, alice, bob, g1, _g2) = battle();
    let round = s.state.combat.as_ref().unwrap().round;
    let act = active(&s);
    // 1–2. A monster goes from its roll to 19 → re-sorted to its place by total.
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: g1,
            total: 8,
        },
    );
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: g1,
            total: 19,
        },
    );
    let o = order(&s);
    assert!(
        o.windows(2).all(|w| w[0].1 >= w[1].1),
        "sorted high→low: {o:?}"
    );
    assert!(o.contains(&(g1, 19)));
    // Negative values are allowed.
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: bob,
            total: -3,
        },
    );
    assert_eq!(order(&s).last().unwrap(), &(bob, -3));
    // 3. A player's initiative during combat.
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: alice,
            total: 25,
        },
    );
    assert_eq!(order(&s)[0], (alice, 25));
    // The active creature keeps its turn, the round and its turn resources are unchanged.
    assert_eq!(active(&s), act);
    let cb = s.state.combat.as_ref().unwrap();
    assert_eq!(cb.round, round);
    assert_eq!(cb.order[cb.index].entity, act);
    // Ties keep the DM's order (stable): two combatants at 10, DM-ordered bob before g1.
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: g1,
            total: 10,
        },
    );
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: bob,
            total: 10,
        },
    );
    let ids: Vec<EntityId> = order(&s).iter().map(|e| e.0).collect();
    let (pb, pg) = (
        ids.iter().position(|x| *x == bob).unwrap(),
        ids.iter().position(|x| *x == g1).unwrap(),
    );
    let mut swapped = ids.clone();
    swapped.swap(pb, pg);
    dm(
        &mut s,
        DmCommand::ReorderInitiative {
            order: swapped.clone(),
        },
    );
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: alice,
            total: 24,
        },
    );
    assert_eq!(order(&s).iter().map(|e| e.0).collect::<Vec<_>>(), swapped);
}

#[test]
fn dm_picks_the_active_combatant_and_can_add_remove_and_reroll() {
    let (mut s, _, alice, bob, g1, g2) = battle();
    let round = s.state.combat.as_ref().unwrap().round;
    // 4. Hand the turn to Bob.
    dm(&mut s, DmCommand::SetActiveCombatant { entity: bob });
    assert_eq!(active(&s), bob);
    assert_eq!(s.state.combat.as_ref().unwrap().round, round);
    // Remove and re-add with a fixed score.
    dm(&mut s, DmCommand::RemoveFromCombat { entity: g2 });
    assert!(!order(&s).iter().any(|e| e.0 == g2));
    dm(
        &mut s,
        DmCommand::AddToCombat {
            entity: g2,
            total: Some(-1),
        },
    );
    assert_eq!(order(&s).last().unwrap(), &(g2, -1));
    assert!(
        s.apply(
            DM,
            Command::Dm(DmCommand::AddToCombat {
                entity: g2,
                total: None
            })
        )
        .is_err(),
        "no duplicates"
    );
    // Reroll one / all: same combatants, still sorted, Bob keeps his turn.
    dm(&mut s, DmCommand::RerollInitiative { entity: Some(g1) });
    dm(&mut s, DmCommand::RerollInitiative { entity: None });
    let o = order(&s);
    assert_eq!(o.len(), 4);
    assert!(o.windows(2).all(|w| w[0].1 >= w[1].1));
    assert!(
        [alice, bob, g1, g2]
            .iter()
            .all(|id| o.iter().any(|e| e.0 == *id))
    );
    assert_eq!(active(&s), bob);
}

#[test]
fn manual_damage_heal_set_max_temp_and_undo() {
    let (mut s, _, alice, _bob, g1, _) = battle();
    // 5. 15 damage to a monster (goblin 10 HP max → raise max first so 15 is meaningful).
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: g1,
            op: HpOp::SetMax(45),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: g1,
            op: HpOp::SetCurrent(45),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: g1,
            op: HpOp::Damage {
                amount: 15,
                rules: true,
                damage_type: None,
            },
        },
    );
    assert_eq!(hp(&s, g1).0, 30);
    // 6. Heal a player by 20, capped at max.
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetMax(60),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetCurrent(30),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Heal { amount: 20 },
        },
    );
    assert_eq!(hp(&s, alice), (50, 60, 0));
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Heal { amount: 99 },
        },
    );
    assert_eq!(hp(&s, alice).0, 60, "regular healing never exceeds max");
    // 7. Set HP directly; never negative, never above max.
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetCurrent(45),
        },
    );
    assert_eq!(hp(&s, alice).0, 45);
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetCurrent(-10),
        },
    );
    assert_eq!(hp(&s, alice).0, 0);
    assert!(
        s.state.creatures[&alice]
            .conditions
            .has(Condition::Unconscious)
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetCurrent(500),
        },
    );
    assert_eq!(hp(&s, alice).0, 60);
    assert!(
        !s.state.creatures[&alice]
            .conditions
            .has(Condition::Unconscious)
    );
    // 8. Max HP lowered below current clamps current.
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetMax(40),
        },
    );
    assert_eq!(hp(&s, alice), (40, 40, 0));
    // 9. Temporary HP absorbs rule damage; direct (override) damage ignores it.
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetTemp(8),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Damage {
                amount: 10,
                rules: true,
                damage_type: None,
            },
        },
    );
    assert_eq!(hp(&s, alice), (38, 40, 0));
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetTemp(5),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Damage {
                amount: 6,
                rules: false,
                damage_type: None,
            },
        },
    );
    assert_eq!(hp(&s, alice), (32, 40, 5));
    // Resistance applies only with rules.
    s.state
        .creatures
        .get_mut(&alice)
        .unwrap()
        .defenses
        .resistances
        .insert(DamageType::Fire);
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::SetTemp(0),
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Damage {
                amount: 10,
                rules: true,
                damage_type: Some(DamageType::Fire),
            },
        },
    );
    assert_eq!(hp(&s, alice).0, 27);
    // 10. Undo the incorrect change.
    dm(&mut s, DmCommand::UndoHp);
    assert_eq!(hp(&s, alice), (32, 40, 0));
    // Undo also brings a killed monster back into initiative.
    let before = order(&s).len();
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: g1,
            op: HpOp::Zero,
        },
    );
    assert!(s.state.creatures[&g1].hp.dead);
    assert_eq!(order(&s).len(), before - 1);
    dm(&mut s, DmCommand::UndoHp);
    assert!(!s.state.creatures[&g1].hp.dead);
    assert_eq!(hp(&s, g1).0, 30);
    assert_eq!(order(&s).len(), before);
    // Bad inputs are rejected and change nothing.
    for op in [
        HpOp::Damage {
            amount: 0,
            rules: true,
            damage_type: None,
        },
        HpOp::Heal { amount: -5 },
        HpOp::SetMax(0),
        HpOp::SetTemp(-1),
    ] {
        let snap = s.state.clone();
        assert!(
            s.apply(DM, Command::Dm(DmCommand::ManualHp { entity: alice, op }))
                .is_err()
        );
        assert_eq!(s.state, snap);
    }
}

#[test]
fn changes_sync_to_players_survive_save_load_and_players_cannot_forge_them() {
    let (mut s, alice_p, alice, _bob, g1, _) = battle();
    dm(
        &mut s,
        DmCommand::SetInitiative {
            entity: g1,
            total: 30,
        },
    );
    dm(
        &mut s,
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Damage {
                amount: 4,
                rules: true,
                damage_type: None,
            },
        },
    );
    // 11. The player's view reflects the new order and HP.
    let v = s.build_view(alice_p, None);
    let cv = v.combat.as_ref().unwrap();
    assert_eq!(cv.order[0].entity, g1);
    assert_eq!(cv.order[0].total, 30);
    let me = v.creatures.iter().find(|c| c.id == alice).unwrap();
    assert!(matches!(me.hp, HpView::Exact { current, .. } if current == hp(&s, alice).0));
    assert!(v.dm.is_none(), "undo info is DM-only");
    assert!(s.build_view(DM, None).dm.unwrap().hp_undo.is_some());
    // 12. Save/load keeps combat order, HP and the undo history.
    let json = serde_json::to_string(&s.state).unwrap();
    let mut back = Session::from_state(
        serde_json::from_str(&json).unwrap(),
        Default::default(),
        Box::new(GameRng::seeded(1)),
    );
    assert_eq!(back.state, s.state);
    back.apply(DM, Command::Dm(DmCommand::UndoHp)).unwrap();
    assert_eq!(
        back.state.creatures[&alice].hp.current,
        back.state.creatures[&alice].hp.max
    );
    // 13. A regular player can't use any of the DM-only controls.
    for c in [
        DmCommand::SetInitiative {
            entity: alice,
            total: 40,
        },
        DmCommand::SetActiveCombatant { entity: alice },
        DmCommand::RerollInitiative { entity: None },
        DmCommand::RemoveFromCombat { entity: g1 },
        DmCommand::ManualHp {
            entity: alice,
            op: HpOp::Restore,
        },
        DmCommand::UndoHp,
    ] {
        let snap = s.state.clone();
        assert!(matches!(
            s.apply(alice_p, Command::Dm(c)),
            Err(CommandError::NotAuthorized)
        ));
        assert_eq!(s.state, snap);
    }
}
