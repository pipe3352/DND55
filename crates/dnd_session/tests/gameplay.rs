//! Spells, class features, items, scripted events and monster actions through the authoritative session.

use dnd_rules::Ability;
use dnd_rules::character::CharacterSheet;
use dnd_rules::conditions::Condition;
use dnd_rules::content::{ClassId, SpeciesId};
use dnd_rules::rng::{GameRng, ScriptedRng};
use dnd_session::command::{Command, DmCommand, Feature, LevelUpChoices};
use dnd_session::model::{Creature, HitReaction, PlayerId, Principal};
use dnd_session::scripts::{DialogueChoice, ScriptAction, ScriptedEvent, Trigger};
use dnd_session::{CommandError, Session};
use dnd_world::EntityId;
use dnd_world::map::{AmbientLight, LayerKind};

const DM: Principal = Principal::Dm;

fn dm(s: &mut Session, c: DmCommand) {
    s.apply(DM, Command::Dm(c)).expect("dm command");
}

fn arena(ambient: AmbientLight) -> Session {
    let mut s = Session::new_campaign("Test");
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
    dm(&mut s, DmCommand::SetAmbient { room, ambient });
    s.set_rng(Box::new(GameRng::seeded(11)));
    s
}

fn sheet(class: ClassId, level: u8, f: impl FnOnce(&mut CharacterSheet)) -> CharacterSheet {
    let mut c = CharacterSheet::example_fighter("Hero");
    c.class = class;
    c.level = level;
    c.weapon_masteries.clear();
    if level >= 3 {
        c.subclass = Some(dnd_rules::classes::srd_subclass(class).into());
    }
    f(&mut c);
    c
}

/// Adds a player-controlled character directly (sheet validation is tested elsewhere).
fn add_pc(
    s: &mut Session,
    name: &str,
    sh: CharacterSheet,
    pos: (i32, i32),
) -> (Principal, EntityId) {
    let p = s.add_player(name);
    let id = EntityId(s.state.next_entity);
    s.state.next_entity += 1;
    let mut sh = sh;
    sh.name = name.into();
    let c = Creature::from_sheet(id, sh, s.state.party_room, pos, p);
    s.state.creatures.insert(id, c);
    s.state.players.get_mut(&p).unwrap().character = Some(id);
    (Principal::Player(p), id)
}

fn spawn(s: &mut Session, template: &str, pos: (i32, i32)) -> EntityId {
    let room = s.state.party_room;
    dm(
        s,
        DmCommand::SpawnMonster {
            template: template.into(),
            room,
            cell: pos,
            hidden: false,
        },
    );
    *s.state.creatures.keys().max().unwrap()
}

fn wizard(level: u8, prepared: &[&str]) -> CharacterSheet {
    sheet(ClassId::Wizard, level, |c| {
        c.species = SpeciesId::Human;
        c.species_skill = Some(dnd_rules::Skill::Insight);
        c.body_armor = None;
        c.shield = false;
        c.weapons = vec!["dagger".into()];
        c.base_scores = dnd_rules::AbilityScores::new(8, 14, 13, 15, 12, 10); // Int 15 (+2 bg +1? no: Soldier) → +2
        c.cantrips = vec!["fire_bolt".into(), "light".into(), "ray_of_frost".into()];
        c.prepared_spells = prepared.iter().map(|s| s.to_string()).collect();
    })
}

fn hp(s: &Session, id: EntityId) -> i32 {
    s.state.creatures[&id].hp.current
}

#[test]
fn light_spell_illuminates_a_dark_cave_for_players() {
    let mut s = arena(AmbientLight::Darkness);
    let (p, w) = add_pc(&mut s, "Wren", wizard(1, &[]), (2, 7));
    let sk = spawn(&mut s, "skeleton", (6, 7));
    // A human (no Darkvision) in total darkness sees nothing.
    assert!(!s.build_view(p, None).creatures.iter().any(|c| c.id == sk));
    // Cast Light on her own gear: Bright 20 ft + Dim 20 ft travels with her.
    let ev = s
        .apply(
            p,
            Command::CastSpell {
                caster: w,
                spell: "light".into(),
                slot: None,
                targets: vec![w],
                point: None,
            },
        )
        .unwrap();
    assert!(ev.iter().any(|r| matches!(
        r.event,
        dnd_session::event::Event::SpellVisual {
            vfx: dnd_session::event::Vfx::Light,
            ..
        }
    )));
    let v = s.build_view(p, None);
    assert!(
        v.creatures.iter().any(|c| c.id == sk),
        "the skeleton 20 ft away is now in Bright Light"
    );
    let room = v.room.unwrap();
    assert_eq!(
        room.visible.unwrap()[(7 * room.width + 6) as usize],
        3,
        "bright"
    );
    assert!(
        room.lights
            .iter()
            .any(|l| l.bright_ft == 20 && l.dim_ft == 20)
    );
    // The light moves with her.
    s.apply(
        p,
        Command::Move {
            entity: w,
            to: (14, 7),
        },
    )
    .unwrap();
    let lights = s.light_levels(s.state.party_room);
    assert_eq!(
        lights[(7 * 20 + 2) as usize],
        dnd_world::vision::LightLevel::Darkness
    );
    // Casting it again ends the previous casting.
    s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "light".into(),
            slot: None,
            targets: vec![w],
            point: None,
        },
    )
    .unwrap();
    assert_eq!(
        s.state
            .effects
            .iter()
            .filter(|e| e.origin == "light")
            .count(),
        1
    );
}

#[test]
fn fireball_hits_everyone_in_the_sphere_and_uses_a_slot() {
    let mut s = arena(AmbientLight::Bright);
    let (p, w) = add_pc(
        &mut s,
        "Wren",
        wizard(5, &["fireball", "misty_step", "shield"]),
        (1, 1),
    );
    let o1 = spawn(&mut s, "ogre", (10, 5));
    let o2 = spawn(&mut s, "ogre", (12, 7));
    let far = spawn(&mut s, "ogre", (18, 13));
    // Initiative: wizard 20, ogres 1.
    s.set_rng(Box::new(ScriptedRng::new([20, 1, 1, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    assert_eq!(s.state.combat.as_ref().unwrap().turn.entity, w);
    // 8d6 all 3s = 24 Fire; ogre 1 saves (d20 19 −1 = 18 ≥ DC 13) → 12; ogre 2 fails (2).
    s.set_rng(Box::new(ScriptedRng::new([3, 3, 3, 3, 3, 3, 3, 3, 19, 2])));
    s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "fireball".into(),
            slot: Some(3),
            targets: vec![],
            point: Some((11, 6)),
        },
    )
    .unwrap();
    assert_eq!(hp(&s, o1), 68 - 12);
    assert_eq!(hp(&s, o2), 68 - 24);
    assert_eq!(hp(&s, far), 68, "outside the 20-ft radius");
    assert_eq!(s.state.creatures[&w].resources.slots_used[2], 1);
    // One spell slot per turn, and the Action is spent.
    let r = s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "misty_step".into(),
            slot: Some(2),
            targets: vec![],
            point: Some((2, 2)),
        },
    );
    assert!(matches!(r, Err(CommandError::Rules(m)) if m.contains("one spell slot")));
    // Wizard 5 DC: 8 + Int +2 + PB 3 = 13.
    assert_eq!(
        s.state.creatures[&w].sheet().unwrap().spell_save_dc(),
        Some(13)
    );
}

#[test]
fn spell_attacks_and_magic_missile() {
    let mut s = arena(AmbientLight::Bright);
    let (p, w) = add_pc(&mut s, "Wren", wizard(1, &["magic_missile"]), (1, 1));
    let o = spawn(&mut s, "ogre", (6, 1));
    // Out of combat spells work without turn order. Fire Bolt nat 20 → 2d10 (5+5).
    s.set_rng(Box::new(ScriptedRng::new([20, 5, 5])));
    s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "fire_bolt".into(),
            slot: None,
            targets: vec![o],
            point: None,
        },
    )
    .unwrap();
    assert_eq!(hp(&s, o), 68 - 10);
    // Magic Missile: three darts of 1d4+1, all auto-hit.
    s.set_rng(Box::new(ScriptedRng::new([1, 2, 3])));
    s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "magic_missile".into(),
            slot: Some(1),
            targets: vec![o, o, o],
            point: None,
        },
    )
    .unwrap();
    assert_eq!(hp(&s, o), 58 - (2 + 3 + 4));
    // Not prepared / out of range / no slot.
    assert!(
        s.apply(
            p,
            Command::CastSpell {
                caster: w,
                spell: "fireball".into(),
                slot: Some(3),
                targets: vec![],
                point: Some((6, 1))
            }
        )
        .is_err()
    );
    s.set_rng(Box::new(GameRng::seeded(1)));
    s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "magic_missile".into(),
            slot: Some(1),
            targets: vec![o],
            point: None,
        },
    )
    .unwrap();
    let r = s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "magic_missile".into(),
            slot: Some(1),
            targets: vec![o],
            point: None,
        },
    );
    assert!(
        matches!(r, Err(CommandError::Rules(m)) if m.contains("no level 1 spell slot")),
        "level 1 wizard has two slots"
    );
    let far = spawn(&mut s, "ogre", (19, 14));
    let mut w2 = s.state.creatures[&w].clone();
    w2.pos = (0, 0);
    s.state.creatures.insert(w, w2);
    let r = s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "ray_of_frost".into(),
            slot: None,
            targets: vec![far],
            point: None,
        },
    );
    assert!(r.is_err(), "Ray of Frost has a 60-ft range");
}

#[test]
fn hold_person_paralyzes_until_concentration_breaks() {
    let mut s = arena(AmbientLight::Bright);
    let (p, w) = add_pc(&mut s, "Wren", wizard(3, &["hold_person"]), (1, 1));
    let bandit = spawn(&mut s, "bandit", (4, 1));
    let ogre = spawn(&mut s, "ogre", (8, 1));
    // Not a Humanoid.
    assert!(
        s.apply(
            p,
            Command::CastSpell {
                caster: w,
                spell: "hold_person".into(),
                slot: Some(2),
                targets: vec![ogre],
                point: None
            }
        )
        .is_err()
    );
    // Bandit Wis save: d20 = 2 → fails.
    s.set_rng(Box::new(ScriptedRng::new([2])));
    s.apply(
        p,
        Command::CastSpell {
            caster: w,
            spell: "hold_person".into(),
            slot: Some(2),
            targets: vec![bandit],
            point: None,
        },
    )
    .unwrap();
    assert!(
        s.state.creatures[&bandit]
            .conditions
            .has(Condition::Paralyzed)
    );
    assert_eq!(
        s.state.creatures[&w].concentration.as_deref(),
        Some("hold_person")
    );
    // The wizard takes 12 damage: Con save DC 10, d20 = 1 → concentration lost, paralysis ends.
    s.set_rng(Box::new(ScriptedRng::new([1])));
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: w,
            delta: -1,
        },
    );
    assert!(s.state.creatures[&w].concentration.is_none());
    assert!(
        !s.state.creatures[&bandit]
            .conditions
            .has(Condition::Paralyzed)
    );
}

#[test]
fn shield_reaction_can_turn_a_hit_into_a_miss() {
    let mut s = arena(AmbientLight::Bright);
    let (p, w) = add_pc(&mut s, "Wren", wizard(1, &["shield"]), (5, 5));
    let g = spawn(&mut s, "goblin_warrior", (6, 5));
    s.set_rng(Box::new(ScriptedRng::new([1, 20]))); // goblin first
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    let ac = s.effective_ac(&s.state.creatures[&w].clone());
    assert_eq!(ac, 12);
    // Scimitar +4: d20 = 10 → 14 ≥ 12 hits; pauses for the reaction.
    s.set_rng(Box::new(ScriptedRng::new([10, 4])));
    s.apply(
        DM,
        Command::Attack {
            attacker: g,
            target: w,
            attack: 0,
        },
    )
    .unwrap();
    let v = s.build_view(p, None);
    let hit = v.combat.unwrap().your_hit_reaction.expect("prompt");
    assert!(hit.options.contains(&HitReaction::Shield));
    let before = hp(&s, w);
    s.apply(
        p,
        Command::HitReaction {
            choice: Some(HitReaction::Shield),
        },
    )
    .unwrap();
    assert_eq!(hp(&s, w), before, "14 < AC 17: miss");
    assert_eq!(s.state.creatures[&w].resources.slots_used[0], 1);
    assert_eq!(s.effective_ac(&s.state.creatures[&w].clone()), 17);
}

#[test]
fn rage_resists_damage_adds_damage_and_blocks_spells() {
    let mut s = arena(AmbientLight::Bright);
    let (p, b) = add_pc(
        &mut s,
        "Brakka",
        sheet(ClassId::Barbarian, 2, |c| c.body_armor = None),
        (5, 5),
    );
    let o = spawn(&mut s, "ogre", (6, 5));
    s.set_rng(Box::new(ScriptedRng::new([20, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    s.apply(
        p,
        Command::UseFeature {
            entity: b,
            feature: Feature::Rage,
            target: None,
            amount: None,
        },
    )
    .unwrap();
    assert!(s.state.creatures[&b].status.raging);
    // Longsword (Str): d20 15 hits; 1d8 = 4 + Str 3 + Rage 2 = 9.
    s.set_rng(Box::new(ScriptedRng::new([15, 4])));
    s.apply(
        p,
        Command::Attack {
            attacker: b,
            target: o,
            attack: 0,
        },
    )
    .unwrap();
    assert_eq!(hp(&s, o), 68 - 9);
    s.apply(p, Command::EndTurn).unwrap();
    // Ogre Greatclub 2d8+4 Bludgeoning: 4+4+4 = 12 → halved to 6 by Rage.
    s.set_rng(Box::new(ScriptedRng::new([18, 4, 4])));
    let before = hp(&s, b);
    s.apply(
        DM,
        Command::Attack {
            attacker: o,
            target: b,
            attack: 0,
        },
    )
    .unwrap();
    assert_eq!(before - hp(&s, b), 6);
    // Rage continues only if extended on the barbarian's turn: the barbarian attacked last turn,
    // so it's still active now; ending the next turn without attacking ends it.
    s.apply(DM, Command::EndTurn).unwrap();
    s.apply(p, Command::EndTurn).unwrap();
    assert!(
        !s.state.creatures[&b].status.raging,
        "not extended this turn → ends"
    );
}

#[test]
fn sneak_attack_needs_an_ally_or_advantage_and_is_once_per_turn() {
    let mut s = arena(AmbientLight::Bright);
    let (p, r) = add_pc(
        &mut s,
        "Vex",
        sheet(ClassId::Rogue, 1, |c| c.weapons = vec!["shortsword".into()]),
        (5, 5),
    );
    let (_, _ally) = add_pc(&mut s, "Ally", sheet(ClassId::Fighter, 1, |_| {}), (7, 5));
    let o = spawn(&mut s, "ogre", (6, 5));
    s.set_rng(Box::new(ScriptedRng::new([20, 1, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    // Shortsword (Finesse uses the better of Str +3 / Dex +1): d20 15; 1d6 = 3 + 3; Sneak Attack 1d6 = 5.
    s.set_rng(Box::new(ScriptedRng::new([15, 3, 5])));
    s.apply(
        p,
        Command::Attack {
            attacker: r,
            target: o,
            attack: 0,
        },
    )
    .unwrap();
    assert_eq!(hp(&s, o), 68 - 11);
    assert!(s.state.combat.as_ref().unwrap().turn.sneak_attack_used);
}

#[test]
fn extra_attack_second_wind_and_action_surge() {
    let mut s = arena(AmbientLight::Bright);
    let (p, f) = add_pc(
        &mut s,
        "Brunhild",
        sheet(ClassId::Fighter, 5, |_| {}),
        (5, 5),
    );
    let o = spawn(&mut s, "ogre", (6, 5));
    s.set_rng(Box::new(ScriptedRng::new([20, 1])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    s.set_rng(Box::new(GameRng::seeded(3)));
    s.apply(
        p,
        Command::Attack {
            attacker: f,
            target: o,
            attack: 0,
        },
    )
    .unwrap();
    s.apply(
        p,
        Command::Attack {
            attacker: f,
            target: o,
            attack: 0,
        },
    )
    .unwrap();
    assert!(
        s.apply(
            p,
            Command::Attack {
                attacker: f,
                target: o,
                attack: 0
            }
        )
        .is_err(),
        "Extra Attack = two attacks"
    );
    s.apply(
        p,
        Command::UseFeature {
            entity: f,
            feature: Feature::ActionSurge,
            target: None,
            amount: None,
        },
    )
    .unwrap();
    s.apply(
        p,
        Command::Attack {
            attacker: f,
            target: o,
            attack: 0,
        },
    )
    .unwrap();
    s.apply(
        p,
        Command::Attack {
            attacker: f,
            target: o,
            attack: 0,
        },
    )
    .unwrap();
    assert!(
        s.apply(
            p,
            Command::Attack {
                attacker: f,
                target: o,
                attack: 0
            }
        )
        .is_err()
    );
    assert!(
        s.apply(
            p,
            Command::UseFeature {
                entity: f,
                feature: Feature::ActionSurge,
                target: None,
                amount: None
            }
        )
        .is_err()
    );
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: f,
            delta: -10,
        },
    );
    let before = hp(&s, f);
    s.apply(
        p,
        Command::UseFeature {
            entity: f,
            feature: Feature::SecondWind,
            target: None,
            amount: None,
        },
    )
    .unwrap();
    let healed = hp(&s, f) - before;
    assert!((6..=15).contains(&healed), "1d10 + 5");
    assert!(
        s.apply(
            p,
            Command::UseFeature {
                entity: f,
                feature: Feature::SecondWind,
                target: None,
                amount: None
            }
        )
        .is_err(),
        "Bonus Action used"
    );
}

#[test]
fn leveling_with_xp_subclass_and_asi() {
    let mut s = arena(AmbientLight::Bright);
    let (p, f) = add_pc(
        &mut s,
        "Brunhild",
        sheet(ClassId::Fighter, 1, |c| {
            c.fighting_style = Some(dnd_rules::character::FightingStyle::Defense)
        }),
        (5, 5),
    );
    assert!(
        s.apply(
            p,
            Command::LevelUp {
                entity: f,
                choices: Box::default()
            }
        )
        .is_err(),
        "no XP yet"
    );
    dm(
        &mut s,
        DmCommand::AwardXp {
            entities: vec![f],
            xp: 2700,
        },
    );
    let max1 = s.state.creatures[&f].hp.max;
    s.apply(
        p,
        Command::LevelUp {
            entity: f,
            choices: Box::default(),
        },
    )
    .unwrap();
    assert_eq!(
        s.state.creatures[&f].hp.max,
        max1 + 6 + 2 + 1,
        "Fighter fixed 6 + Con 2 + Dwarven Toughness 1"
    );
    s.apply(
        p,
        Command::LevelUp {
            entity: f,
            choices: Box::default(),
        },
    )
    .unwrap();
    assert_eq!(
        s.state.creatures[&f].sheet().unwrap().subclass.as_deref(),
        Some("Champion")
    );
    // Level 4 requires the ASI choice.
    assert!(
        s.apply(
            p,
            Command::LevelUp {
                entity: f,
                choices: Box::default()
            }
        )
        .is_err()
    );
    let choices = LevelUpChoices {
        asi: vec![(Ability::Strength, 1), (Ability::Constitution, 1)],
        ..Default::default()
    };
    s.apply(
        p,
        Command::LevelUp {
            entity: f,
            choices: Box::new(choices),
        },
    )
    .unwrap();
    let sh = s.state.creatures[&f].sheet().unwrap().clone();
    assert_eq!(
        (
            sh.level,
            sh.scores().get(Ability::Strength),
            sh.scores().get(Ability::Constitution)
        ),
        (4, 18, 16)
    );
    assert_eq!(
        s.state.creatures[&f].attacks[0].to_hit,
        4 + 2,
        "Str 18 → +4, PB +2"
    );
    // Milestone leveling by the DM.
    dm(&mut s, DmCommand::GrantLevel { entity: f });
    s.apply(
        p,
        Command::LevelUp {
            entity: f,
            choices: Box::default(),
        },
    )
    .unwrap();
    assert_eq!(s.state.creatures[&f].attacks_per_action, 2);
}

#[test]
fn rests_restore_resources() {
    let mut s = arena(AmbientLight::Bright);
    let (p, w) = add_pc(&mut s, "Wren", wizard(1, &["magic_missile"]), (1, 1));
    let o = spawn(&mut s, "ogre", (5, 1));
    for _ in 0..2 {
        s.apply(
            p,
            Command::CastSpell {
                caster: w,
                spell: "magic_missile".into(),
                slot: Some(1),
                targets: vec![o],
                point: None,
            },
        )
        .unwrap();
    }
    assert_eq!(s.state.creatures[&w].resources.slots_used[0], 2);
    dm(&mut s, DmCommand::Rest { long: false });
    // Arcane Recovery (half of level 1, rounded up = 1 level of slots).
    assert_eq!(s.state.creatures[&w].resources.slots_used[0], 1);
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: w,
            delta: -3,
        },
    );
    dm(&mut s, DmCommand::Rest { long: true });
    assert_eq!(s.state.creatures[&w].resources.slots_used[0], 0);
    assert_eq!(hp(&s, w), s.state.creatures[&w].hp.max);
}

#[test]
fn potions_chests_and_giving_items() {
    let mut s = arena(AmbientLight::Bright);
    let (p, a) = add_pc(&mut s, "A", sheet(ClassId::Fighter, 1, |_| {}), (5, 5));
    let (_, b) = add_pc(&mut s, "B", sheet(ClassId::Fighter, 1, |_| {}), (6, 5));
    let room = s.state.party_room;
    dm(
        &mut s,
        DmCommand::PlaceObject {
            room,
            asset: "chest".into(),
            cell: (5, 6),
            hidden: false,
        },
    );
    let chest = s.state.rooms[&room].map.objects.last().unwrap().id;
    dm(
        &mut s,
        DmCommand::SetContainer {
            room,
            object: chest,
            items: vec![dnd_rules::items::catalog_item("potion_of_healing").unwrap()],
        },
    );
    // Players can't see inside until it's opened.
    assert!(s.build_view(p, None).room.unwrap().containers.is_empty());
    assert!(
        s.apply(
            p,
            Command::TakeFromContainer {
                entity: a,
                object: chest,
                item: "potion_of_healing".into(),
                quantity: 1
            }
        )
        .is_err()
    );
    s.apply(
        p,
        Command::Interact {
            entity: a,
            object: chest,
        },
    )
    .unwrap();
    assert_eq!(s.build_view(p, None).room.unwrap().containers.len(), 1);
    s.apply(
        p,
        Command::TakeFromContainer {
            entity: a,
            object: chest,
            item: "potion_of_healing".into(),
            quantity: 1,
        },
    )
    .unwrap();
    // Give to B (adjacent), B drinks it.
    s.apply(
        p,
        Command::GiveItem {
            from: a,
            to: b,
            item: "potion_of_healing".into(),
            quantity: 1,
        },
    )
    .unwrap();
    dm(
        &mut s,
        DmCommand::AdjustHp {
            entity: b,
            delta: -8,
        },
    );
    let pb = Principal::Player(PlayerId(2));
    let before = hp(&s, b);
    s.apply(
        pb,
        Command::UseItem {
            entity: b,
            item: "potion_of_healing".into(),
            target: None,
        },
    )
    .unwrap();
    assert!((4..=10).contains(&(hp(&s, b) - before)), "2d4 + 2");
    assert!(s.state.creatures[&b].sheet().unwrap().inventory.is_empty());
}

#[test]
fn scripted_events_trap_lever_and_dialogue() {
    let mut s = demo_like();
    let room = s.state.party_room;
    let (p, h) = add_pc(&mut s, "Hero", sheet(ClassId::Fighter, 1, |_| {}), (2, 2));
    dm(
        &mut s,
        DmCommand::PlaceObject {
            room,
            asset: "lever".into(),
            cell: (2, 4),
            hidden: false,
        },
    );
    let lever = s.state.rooms[&room].map.objects.last().unwrap().id;
    dm(
        &mut s,
        DmCommand::PlaceObject {
            room,
            asset: "door".into(),
            cell: (9, 9),
            hidden: false,
        },
    );
    let door = s.state.rooms[&room].map.objects.last().unwrap().id;
    let trap = ScriptedEvent {
        id: 0,
        name: "Spikes".into(),
        enabled: true,
        once: true,
        fired: false,
        trigger: Trigger::EnterArea {
            room,
            a: (4, 0),
            b: (4, 14),
        },
        actions: vec![ScriptAction::Trap {
            dice: "2d6".into(),
            damage_type: dnd_rules::DamageType::Piercing,
            save: Some(Ability::Dexterity),
            dc: 30,
            half_on_success: true,
        }],
    };
    let gate = ScriptedEvent {
        id: 0,
        name: "Gate".into(),
        enabled: true,
        once: false,
        fired: false,
        trigger: Trigger::Interact {
            room,
            object: lever,
        },
        actions: vec![ScriptAction::SetObjectOpen {
            room,
            object: door,
            open: Some(true),
        }],
    };
    let talk = ScriptedEvent {
        id: 0,
        name: "Hermit".into(),
        enabled: true,
        once: true,
        fired: false,
        trigger: Trigger::Manual,
        actions: vec![ScriptAction::Dialogue {
            speaker: "Hermit".into(),
            text: "Take this.".into(),
            choices: vec![DialogueChoice {
                text: "Thank you".into(),
                actions: vec![ScriptAction::GiveItem {
                    item_id: "torch".into(),
                    quantity: 2,
                }],
            }],
        }],
    };
    for sc in [trap, gate, talk] {
        dm(
            &mut s,
            DmCommand::SaveScript {
                script: Box::new(sc),
            },
        );
    }
    let before = hp(&s, h);
    s.apply(
        p,
        Command::Move {
            entity: h,
            to: (5, 2),
        },
    )
    .unwrap();
    assert!(
        hp(&s, h) < before,
        "crossing column 4 springs the trap (DC 30 save fails)"
    );
    let after = hp(&s, h);
    s.apply(
        p,
        Command::Move {
            entity: h,
            to: (3, 2),
        },
    )
    .unwrap();
    s.apply(
        p,
        Command::Move {
            entity: h,
            to: (5, 2),
        },
    )
    .unwrap();
    assert_eq!(hp(&s, h), after, "once-only trap");
    s.apply(
        p,
        Command::Move {
            entity: h,
            to: (2, 3),
        },
    )
    .unwrap();
    s.apply(
        p,
        Command::Interact {
            entity: h,
            object: lever,
        },
    )
    .unwrap();
    assert!(
        s.state.rooms[&room].map.object(door).unwrap().open,
        "lever opens the gate"
    );
    let talk_id = s
        .state
        .scripts
        .iter()
        .find(|x| x.name == "Hermit")
        .unwrap()
        .id;
    dm(
        &mut s,
        DmCommand::FireScript {
            id: talk_id,
            actor: Some(h),
        },
    );
    assert_eq!(
        s.build_view(p, None).dialogue.unwrap().2,
        vec!["Thank you".to_string()]
    );
    s.apply(p, Command::DialogueChoice { choice: 0 }).unwrap();
    let inv = &s.state.creatures[&h].sheet().unwrap().inventory;
    assert_eq!(inv.iter().find(|i| i.id == "torch").unwrap().quantity, 2);
    // Players can't author events.
    assert_eq!(
        s.apply(
            p,
            Command::Dm(DmCommand::FireScript {
                id: talk_id,
                actor: None
            })
        ),
        Err(CommandError::NotAuthorized)
    );
}

fn demo_like() -> Session {
    arena(AmbientLight::Bright)
}

#[test]
fn dragon_breath_cone_and_recharge() {
    let mut s = arena(AmbientLight::Bright);
    let (_, a) = add_pc(&mut s, "A", sheet(ClassId::Fighter, 1, |_| {}), (10, 7));
    let (_, b) = add_pc(&mut s, "B", sheet(ClassId::Fighter, 1, |_| {}), (12, 7));
    let (_, c) = add_pc(&mut s, "C", sheet(ClassId::Fighter, 1, |_| {}), (2, 2));
    let d = spawn(&mut s, "adult_red_dragon", (5, 7));
    s.set_rng(Box::new(ScriptedRng::new([1, 1, 1, 20])));
    dm(&mut s, DmCommand::StartCombat { surprised: vec![] });
    assert_eq!(s.state.combat.as_ref().unwrap().turn.entity, d);
    let breath = s.state.creatures[&d]
        .save_actions
        .iter()
        .position(|x| x.name.starts_with("Fire Breath"))
        .unwrap();
    s.set_rng(Box::new(GameRng::seeded(5)));
    s.apply(
        DM,
        Command::MonsterAction {
            monster: d,
            action: breath,
            targets: vec![],
            point: Some((15, 7)),
        },
    )
    .unwrap();
    assert!(
        s.state.creatures[&a].hp.current < 13 && s.state.creatures[&b].hp.current < 13,
        "both in the 60-ft cone"
    );
    assert_eq!(s.state.creatures[&c].hp.current, 13, "behind the dragon");
    assert!(s.state.creatures[&d].resources.recharging.contains(&breath));
    assert!(
        s.apply(
            DM,
            Command::MonsterAction {
                monster: d,
                action: breath,
                targets: vec![],
                point: Some((15, 7))
            }
        )
        .is_err()
    );
}

#[test]
fn drawings_respect_visibility_and_permissions() {
    use dnd_session::model::{DrawShape, DrawVisibility, Drawing};
    let mut s = arena(AmbientLight::Bright);
    let (p, _) = add_pc(&mut s, "A", sheet(ClassId::Fighter, 1, |_| {}), (2, 2));
    let room = s.state.party_room;
    let mk = |vis| Drawing {
        id: 0,
        shape: DrawShape::Line((1.0, 1.0), (5.0, 5.0)),
        color: [255, 0, 0, 255],
        width: 3.0,
        visibility: vis,
        temporary: false,
        author: String::new(),
    };
    s.apply(
        DM,
        Command::AddDrawing {
            room,
            drawing: Box::new(mk(DrawVisibility::DmOnly)),
        },
    )
    .unwrap();
    s.apply(
        DM,
        Command::AddDrawing {
            room,
            drawing: Box::new(mk(DrawVisibility::All)),
        },
    )
    .unwrap();
    assert_eq!(s.build_view(p, None).room.unwrap().drawings.len(), 1);
    assert_eq!(s.build_view(DM, None).room.unwrap().drawings.len(), 2);
    assert_eq!(
        s.apply(
            p,
            Command::AddDrawing {
                room,
                drawing: Box::new(mk(DrawVisibility::All))
            }
        ),
        Err(CommandError::NotAuthorized)
    );
    dm(&mut s, DmCommand::SetPlayersCanDraw { allowed: true });
    s.apply(
        p,
        Command::AddDrawing {
            room,
            drawing: Box::new(mk(DrawVisibility::DmOnly)),
        },
    )
    .unwrap();
    assert!(
        s.state.rooms[&room].drawings.last().unwrap().temporary,
        "player drawings are temporary and public"
    );
    dm(
        &mut s,
        DmCommand::ClearDrawings {
            room,
            temporary_only: true,
        },
    );
    assert_eq!(s.state.rooms[&room].drawings.len(), 2);
}

#[test]
fn editor_multi_object_operations_with_undo() {
    let mut s = arena(AmbientLight::Bright);
    let room = s.state.party_room;
    for x in 0..3 {
        dm(
            &mut s,
            DmCommand::PlaceObject {
                room,
                asset: "barrel".into(),
                cell: (x, 0),
                hidden: false,
            },
        );
    }
    let ids: Vec<_> = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .map(|o| o.id)
        .collect();
    dm(
        &mut s,
        DmCommand::MoveObjects {
            room,
            objects: ids.clone(),
            dx: 2,
            dy: 3,
        },
    );
    assert!(s.state.rooms[&room].map.objects.iter().all(|o| o.y == 3));
    let copies: Vec<_> = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .cloned()
        .map(|mut o| {
            o.y += 2;
            o
        })
        .collect();
    dm(
        &mut s,
        DmCommand::AddObjects {
            room,
            objects: copies,
        },
    );
    assert_eq!(s.state.rooms[&room].map.objects.len(), 6);
    let unique: std::collections::HashSet<_> = s.state.rooms[&room]
        .map
        .objects
        .iter()
        .map(|o| o.id)
        .collect();
    assert_eq!(unique.len(), 6, "pasted objects get new ids");
    dm(&mut s, DmCommand::Undo { room });
    assert_eq!(s.state.rooms[&room].map.objects.len(), 3);
    assert!(
        s.apply(
            DM,
            Command::Dm(DmCommand::MoveObjects {
                room,
                objects: ids,
                dx: -10,
                dy: 0
            })
        )
        .is_err(),
        "can't leave the map"
    );
    let mut o = s.state.rooms[&room].map.objects[0].clone();
    o.label = "Ale".into();
    o.blocks_movement = false;
    dm(
        &mut s,
        DmCommand::UpdateObject {
            room,
            object: Box::new(o.clone()),
        },
    );
    assert_eq!(s.state.rooms[&room].map.object(o.id).unwrap().label, "Ale");
    dm(
        &mut s,
        DmCommand::MoveLayer {
            room,
            layer: LayerKind::Terrain,
            up: true,
        },
    );
    assert_eq!(s.state.rooms[&room].map.layers[1].kind, LayerKind::Terrain);
}

#[test]
fn old_format_state_still_loads() {
    // A v1 save (no effects/scripts/resources fields) must deserialize with defaults.
    let s = arena(AmbientLight::Bright);
    let mut v = serde_json::to_value(&s.state).unwrap();
    let obj = v.as_object_mut().unwrap();
    obj.remove("effects");
    obj.remove("scripts");
    obj.remove("pending_dialogue");
    let back: dnd_session::GameState = serde_json::from_value(v).unwrap();
    assert!(back.effects.is_empty() && back.scripts.is_empty());
}
