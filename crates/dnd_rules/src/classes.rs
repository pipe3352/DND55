//! Class progression (SRD 5.2.1 class feature tables and feature descriptions), spellcasting
//! progression, and the numbers class features need (rage damage, sneak attack dice, ...).
//!
//! Data: `content/srd_class_tables.json` and `content/srd_class_features.json`
//! (extracted by `tools/extract_class_tables.py` / `tools/extract_class_features.py`).

use crate::ability::Ability;
use crate::content::ClassId;
use crate::dice::DiceExpr;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Row {
    pub level: u8,
    pub pb: u8,
    pub features: String,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Feature {
    pub level: u8,
    pub name: String,
    pub subclass: Option<String>,
    pub text: String,
}

static TABLES: OnceLock<HashMap<String, Vec<Row>>> = OnceLock::new();
static FEATURES: OnceLock<HashMap<String, Vec<Feature>>> = OnceLock::new();

fn tables() -> &'static HashMap<String, Vec<Row>> {
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!("../../../content/srd_class_tables.json"))
            .expect("class tables")
    })
}

fn all_features() -> &'static HashMap<String, Vec<Feature>> {
    FEATURES.get_or_init(|| {
        serde_json::from_str(include_str!("../../../content/srd_class_features.json"))
            .expect("class features")
    })
}

pub fn class_name(c: ClassId) -> &'static str {
    crate::content::class(c).name
}

pub fn row(c: ClassId, level: u8) -> &'static Row {
    &tables()[class_name(c)][(level.clamp(1, 20) - 1) as usize]
}

fn num(s: &str) -> u8 {
    s.trim_start_matches('+').parse().unwrap_or(0)
}

/// The single SRD subclass for each class.
pub fn srd_subclass(c: ClassId) -> &'static str {
    match c {
        ClassId::Barbarian => "Path of the Berserker",
        ClassId::Bard => "College of Lore",
        ClassId::Cleric => "Life Domain",
        ClassId::Druid => "Circle of the Land",
        ClassId::Fighter => "Champion",
        ClassId::Monk => "Warrior of the Open Hand",
        ClassId::Paladin => "Oath of Devotion",
        ClassId::Ranger => "Hunter",
        ClassId::Rogue => "Thief",
        ClassId::Sorcerer => "Draconic Sorcery",
        ClassId::Warlock => "Fiend Patron",
        ClassId::Wizard => "Evoker",
    }
}

/// Features gained at or below `level` (subclass features included once the subclass is chosen).
pub fn features(c: ClassId, level: u8, subclass: Option<&str>) -> Vec<&'static Feature> {
    all_features()[class_name(c)]
        .iter()
        .filter(|f| f.level <= level && (f.subclass.is_none() || f.subclass.as_deref() == subclass))
        .collect()
}

pub fn has_feature(c: ClassId, level: u8, subclass: Option<&str>, name: &str) -> bool {
    features(c, level, subclass)
        .iter()
        .any(|f| f.name.starts_with(name))
}

/// XP needed to reach each level (SRD "Character Advancement" table).
pub const XP_THRESHOLDS: [u32; 20] = [
    0, 300, 900, 2_700, 6_500, 14_000, 23_000, 34_000, 48_000, 64_000, 85_000, 100_000, 120_000,
    140_000, 165_000, 195_000, 225_000, 265_000, 305_000, 355_000,
];

pub fn level_for_xp(xp: u32) -> u8 {
    XP_THRESHOLDS
        .iter()
        .rposition(|t| xp >= *t)
        .map(|i| i as u8 + 1)
        .unwrap_or(1)
}

/// Levels that grant the Ability Score Improvement feat (or another feat).
pub fn is_asi_level(c: ClassId, level: u8) -> bool {
    row(c, level).features.contains("Ability Score Improvement")
}

// ------------------------------------------------------------------ spellcasting

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CasterKind {
    Full,
    Half,
    Pact,
}

pub fn spellcasting(c: ClassId) -> Option<(Ability, CasterKind)> {
    use Ability::*;
    Some(match c {
        ClassId::Bard => (Charisma, CasterKind::Full),
        ClassId::Cleric => (Wisdom, CasterKind::Full),
        ClassId::Druid => (Wisdom, CasterKind::Full),
        ClassId::Sorcerer => (Charisma, CasterKind::Full),
        ClassId::Wizard => (Intelligence, CasterKind::Full),
        ClassId::Paladin => (Charisma, CasterKind::Half),
        ClassId::Ranger => (Wisdom, CasterKind::Half),
        ClassId::Warlock => (Charisma, CasterKind::Pact),
        _ => return None,
    })
}

/// Column index of the "Cantrips" and "Prepared Spells" columns in each caster's table.
fn caster_columns(c: ClassId) -> Option<(Option<usize>, usize, usize)> {
    // (cantrips col, prepared col, first slot col)
    Some(match c {
        ClassId::Wizard => (Some(0), 1, 2),
        ClassId::Bard | ClassId::Cleric | ClassId::Druid | ClassId::Sorcerer => (Some(1), 2, 3),
        ClassId::Paladin | ClassId::Ranger => (None, 1, 2),
        ClassId::Warlock => (Some(1), 2, 3),
        _ => return None,
    })
}

pub fn cantrips_known(c: ClassId, level: u8) -> u8 {
    caster_columns(c)
        .and_then(|(cc, _, _)| cc)
        .map(|i| num(&row(c, level).columns[i]))
        .unwrap_or(0)
}

pub fn prepared_spells(c: ClassId, level: u8) -> u8 {
    caster_columns(c)
        .map(|(_, p, _)| num(&row(c, level).columns[p]))
        .unwrap_or(0)
}

/// Spell slots per spell level 1–9. Warlocks use [`pact_slots`] instead (returned here as all zero).
pub fn spell_slots(c: ClassId, level: u8) -> [u8; 9] {
    let mut out = [0u8; 9];
    match caster_columns(c) {
        Some((_, _, first)) if c != ClassId::Warlock => {
            for (i, v) in row(c, level).columns[first..].iter().take(9).enumerate() {
                out[i] = num(v);
            }
        }
        _ => {}
    }
    out
}

/// Warlock Pact Magic: (number of slots, slot level). Regained on a Short Rest.
pub fn pact_slots(level: u8) -> (u8, u8) {
    let r = row(ClassId::Warlock, level);
    (num(&r.columns[3]), num(&r.columns[4]))
}

/// Highest spell level the character can cast with a slot.
pub fn max_spell_level(c: ClassId, level: u8) -> u8 {
    if c == ClassId::Warlock {
        return pact_slots(level).1;
    }
    spell_slots(c, level)
        .iter()
        .rposition(|n| *n > 0)
        .map(|i| i as u8 + 1)
        .unwrap_or(0)
}

/// Spells the subclass always has prepared (SRD subclass spell tables), by class level.
pub fn subclass_always_prepared(c: ClassId, level: u8) -> Vec<&'static str> {
    let table: &[(u8, &[&str])] = match c {
        ClassId::Cleric => &[
            (3, &["aid", "bless", "cure_wounds", "lesser_restoration"]),
            (5, &["mass_healing_word", "revivify"]),
            (7, &["aura_of_life", "death_ward"]),
            (9, &["greater_restoration", "mass_cure_wounds"]),
        ],
        ClassId::Paladin => &[
            (3, &["protection_from_evil_and_good", "shield_of_faith"]),
            (5, &["aid", "zone_of_truth"]),
            (9, &["beacon_of_hope", "dispel_magic"]),
            (13, &["freedom_of_movement", "guardian_of_faith"]),
            (17, &["commune", "flame_strike"]),
        ],
        _ => &[],
    };
    let mut v: Vec<&str> = table
        .iter()
        .filter(|(l, _)| level >= *l)
        .flat_map(|(_, s)| s.iter().copied())
        .collect();
    if c == ClassId::Paladin && level >= 2 {
        v.push("divine_smite"); // Paladin's Smite: always prepared
    }
    if c == ClassId::Ranger {
        v.push("hunter_s_mark"); // Favored Enemy: always prepared
    }
    v
}

// ------------------------------------------------------------------ martial numbers

/// Barbarian: (rages per long rest, rage damage bonus).
pub fn rage(level: u8) -> (u8, i32) {
    let r = row(ClassId::Barbarian, level);
    (num(&r.columns[0]), num(&r.columns[1]) as i32)
}

pub fn sneak_attack_dice(level: u8) -> DiceExpr {
    DiceExpr::parse(&row(ClassId::Rogue, level).columns[0]).unwrap_or_else(|_| DiceExpr::dice(1, 6))
}

pub fn martial_arts_die(level: u8) -> u32 {
    row(ClassId::Monk, level).columns[0]
        .trim_start_matches("1d")
        .parse()
        .unwrap_or(6)
}

pub fn focus_points(level: u8) -> u8 {
    if level < 2 { 0 } else { level }
}

/// Monk Unarmored Movement bonus (Monk Features table).
pub fn unarmored_movement(level: u8) -> i32 {
    match level {
        0..=1 => 0,
        2..=5 => 10,
        6..=9 => 15,
        10..=13 => 20,
        14..=17 => 25,
        _ => 30,
    }
}

pub fn second_wind_uses(level: u8) -> u8 {
    num(&row(ClassId::Fighter, level).columns[0])
}

pub fn weapon_mastery_count(c: ClassId, level: u8) -> u8 {
    match c {
        ClassId::Barbarian => num(&row(c, level).columns[2]),
        ClassId::Fighter => num(&row(c, level).columns[1]),
        ClassId::Paladin | ClassId::Ranger | ClassId::Rogue => 2,
        _ => 0,
    }
}

pub fn channel_divinity_uses(c: ClassId, level: u8) -> u8 {
    match c {
        ClassId::Cleric | ClassId::Paladin => num(&row(c, level).columns[0]),
        _ => 0,
    }
}

pub fn bardic_inspiration_die(level: u8) -> u32 {
    row(ClassId::Bard, level).columns[0]
        .trim_start_matches(['D', 'd'])
        .parse()
        .unwrap_or(6)
}

pub fn action_surge_uses(level: u8) -> u8 {
    match level {
        0..=1 => 0,
        2..=16 => 1,
        _ => 2,
    }
}

/// Attacks per Attack action (Extra Attack features).
pub fn attacks_per_action(c: ClassId, level: u8) -> u8 {
    match c {
        ClassId::Fighter => match level {
            0..=4 => 1,
            5..=10 => 2,
            11..=19 => 3,
            _ => 4,
        },
        ClassId::Barbarian | ClassId::Monk | ClassId::Paladin | ClassId::Ranger if level >= 5 => 2,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spell_slots_match_srd_tables() {
        assert_eq!(spell_slots(ClassId::Wizard, 1), [2, 0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(spell_slots(ClassId::Wizard, 5), [4, 3, 2, 0, 0, 0, 0, 0, 0]);
        assert_eq!(
            spell_slots(ClassId::Wizard, 20),
            [4, 3, 3, 3, 3, 2, 2, 1, 1]
        );
        assert_eq!(
            spell_slots(ClassId::Paladin, 1),
            [2, 0, 0, 0, 0, 0, 0, 0, 0],
            "2024 Paladins cast at level 1"
        );
        assert_eq!(spell_slots(ClassId::Ranger, 9), [4, 3, 2, 0, 0, 0, 0, 0, 0]);
        assert_eq!(spell_slots(ClassId::Fighter, 20), [0; 9]);
        assert_eq!(
            (
                cantrips_known(ClassId::Wizard, 1),
                prepared_spells(ClassId::Wizard, 1)
            ),
            (3, 4)
        );
        assert_eq!(
            (
                cantrips_known(ClassId::Cleric, 4),
                prepared_spells(ClassId::Cleric, 4)
            ),
            (4, 7)
        );
        assert_eq!(pact_slots(1), (1, 1));
        assert_eq!(pact_slots(5), (2, 3));
        assert_eq!(pact_slots(17), (4, 5));
        assert_eq!(max_spell_level(ClassId::Wizard, 17), 9);
        assert_eq!(max_spell_level(ClassId::Warlock, 9), 5);
    }

    #[test]
    fn martial_progressions() {
        assert_eq!(rage(1), (2, 2));
        assert_eq!(rage(9), (4, 3));
        assert_eq!(rage(16), (5, 4));
        assert_eq!(sneak_attack_dice(1).to_string(), "1d6");
        assert_eq!(sneak_attack_dice(19).to_string(), "10d6");
        assert_eq!(
            (
                martial_arts_die(1),
                martial_arts_die(5),
                martial_arts_die(17)
            ),
            (6, 8, 12)
        );
        assert_eq!(
            (
                second_wind_uses(1),
                second_wind_uses(4),
                second_wind_uses(10)
            ),
            (2, 3, 4)
        );
        assert_eq!(
            (
                attacks_per_action(ClassId::Fighter, 4),
                attacks_per_action(ClassId::Fighter, 5),
                attacks_per_action(ClassId::Fighter, 11),
                attacks_per_action(ClassId::Fighter, 20)
            ),
            (1, 2, 3, 4)
        );
        assert_eq!(attacks_per_action(ClassId::Wizard, 20), 1);
        assert_eq!(
            (
                weapon_mastery_count(ClassId::Fighter, 1),
                weapon_mastery_count(ClassId::Fighter, 16)
            ),
            (3, 6)
        );
        assert_eq!(bardic_inspiration_die(5), 8);
        assert_eq!((unarmored_movement(2), unarmored_movement(18)), (10, 30));
    }

    #[test]
    fn features_unlock_at_their_levels() {
        assert!(has_feature(ClassId::Fighter, 2, None, "Action Surge"));
        assert!(!has_feature(ClassId::Fighter, 1, None, "Action Surge"));
        assert!(has_feature(
            ClassId::Fighter,
            3,
            Some("Champion"),
            "Improved Critical"
        ));
        assert!(!has_feature(
            ClassId::Fighter,
            2,
            Some("Champion"),
            "Improved Critical"
        ));
        assert!(has_feature(
            ClassId::Barbarian,
            3,
            Some("Path of the Berserker"),
            "Frenzy"
        ));
        assert!(has_feature(ClassId::Rogue, 5, None, "Uncanny Dodge"));
        assert!(
            is_asi_level(ClassId::Fighter, 6),
            "Fighters get an extra ASI at 6"
        );
        assert!(!is_asi_level(ClassId::Wizard, 6));
        for c in crate::content::CLASSES {
            assert!(
                !features(c.id, 20, Some(srd_subclass(c.id))).is_empty(),
                "{}",
                c.name
            );
        }
    }

    #[test]
    fn xp_levels() {
        assert_eq!(level_for_xp(0), 1);
        assert_eq!(level_for_xp(299), 1);
        assert_eq!(level_for_xp(300), 2);
        assert_eq!(level_for_xp(6_500), 5);
        assert_eq!(level_for_xp(400_000), 20);
    }
}
