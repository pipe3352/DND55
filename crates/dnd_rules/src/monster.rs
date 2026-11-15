//! Monster stat blocks (2024 format) and the starter SRD 5.2.1 bestiary.
//!
//! Only stat blocks transcribed and checked against SRD 5.2.1 are included; the bestiary grows
//! through data files (`content/` packs) — see `docs/DESIGN.md` §7.

use crate::ability::AbilityScores;
use crate::attack::{AttackProfile, DamagePart};
use crate::conditions::Condition;
use crate::content::Size;
use crate::damage::{DamageType, Defenses};
use crate::dice::DiceExpr;
use crate::equipment::WeaponKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MonsterTemplate {
    pub id: String,
    pub name: String,
    pub size: Size,
    pub creature_type: String,
    pub alignment: String,
    pub armor_class: i32,
    pub hit_points: i32,
    pub hit_dice: DiceExpr,
    pub speed_ft: i32,
    /// 2024 stat blocks list Initiative as a modifier and a score (e.g. "+2 (12)").
    pub initiative_bonus: i32,
    pub scores: AbilityScores,
    pub defenses: Defenses,
    pub condition_immunities: BTreeSet<Condition>,
    pub darkvision_ft: u32,
    pub passive_perception: i32,
    /// Challenge Rating as text ("1/4"), XP and Proficiency Bonus as printed.
    pub challenge: String,
    pub xp: u32,
    pub proficiency_bonus: i32,
    pub attacks: Vec<AttackProfile>,
    /// Traits/actions not yet automated, shown to the DM as text.
    pub notes: Vec<String>,
    /// Marks provenance: "srd-5.2.1", "homebrew", ...
    pub source: String,
}

impl MonsterTemplate {
    pub fn initiative_score(&self) -> i32 {
        10 + self.initiative_bonus
    }
}

fn melee(name: &str, to_hit: i32, dice: DiceExpr, t: DamageType, ability_mod: i32) -> AttackProfile {
    AttackProfile {
        name: name.into(),
        kind: WeaponKind::Melee,
        to_hit,
        damage: vec![DamagePart { dice, damage_type: t }],
        extra_damage_with_advantage: vec![],
        reach_ft: 5,
        range_ft: None,
        mastery: None,
        ability_mod,
    }
}

fn ranged(name: &str, to_hit: i32, range: (u32, u32), dice: DiceExpr, t: DamageType, ability_mod: i32) -> AttackProfile {
    AttackProfile { kind: WeaponKind::Ranged, reach_ft: 0, range_ft: Some(range), ..melee(name, to_hit, dice, t, ability_mod) }
}

/// Starter bestiary transcribed from SRD 5.2.1.
pub fn srd_bestiary() -> Vec<MonsterTemplate> {
    let mut goblin_scimitar = melee("Scimitar", 4, DiceExpr::dice(1, 6).with_modifier(2), DamageType::Slashing, 2);
    goblin_scimitar.extra_damage_with_advantage = vec![DamagePart { dice: DiceExpr::dice(1, 4), damage_type: DamageType::Slashing }];
    let mut goblin_bow = ranged("Shortbow", 4, (80, 320), DiceExpr::dice(1, 6).with_modifier(2), DamageType::Piercing, 2);
    goblin_bow.extra_damage_with_advantage = vec![DamagePart { dice: DiceExpr::dice(1, 4), damage_type: DamageType::Piercing }];

    vec![
        MonsterTemplate {
            id: "goblin_warrior".into(),
            name: "Goblin Warrior".into(),
            size: Size::Small,
            creature_type: "Fey (Goblinoid)".into(),
            alignment: "Chaotic Neutral".into(),
            armor_class: 15,
            hit_points: 10,
            hit_dice: DiceExpr::dice(3, 6),
            speed_ft: 30,
            initiative_bonus: 2,
            scores: AbilityScores::new(8, 15, 10, 10, 8, 8),
            defenses: Defenses::default(),
            condition_immunities: BTreeSet::new(),
            darkvision_ft: 60,
            passive_perception: 9,
            challenge: "1/4".into(),
            xp: 50,
            proficiency_bonus: 2,
            attacks: vec![goblin_scimitar, goblin_bow],
            notes: vec![
                "Skills: Stealth +6".into(),
                "Bonus Action — Nimble Escape: the goblin takes the Disengage or Hide action.".into(),
                "Reaction — Redirect Attack (not yet automated).".into(),
            ],
            source: "srd-5.2.1".into(),
        },
        MonsterTemplate {
            id: "skeleton".into(),
            name: "Skeleton".into(),
            size: Size::Medium,
            creature_type: "Undead".into(),
            alignment: "Lawful Evil".into(),
            armor_class: 14,
            hit_points: 13,
            hit_dice: DiceExpr::dice(2, 8).with_modifier(4),
            speed_ft: 30,
            initiative_bonus: 3,
            scores: AbilityScores::new(10, 16, 15, 6, 8, 5),
            defenses: Defenses {
                vulnerabilities: [DamageType::Bludgeoning].into(),
                immunities: [DamageType::Poison].into(),
                ..Default::default()
            },
            condition_immunities: [Condition::Poisoned].into(),
            darkvision_ft: 60,
            passive_perception: 9,
            challenge: "1/4".into(),
            xp: 50,
            proficiency_bonus: 2,
            attacks: vec![
                melee("Shortsword", 5, DiceExpr::dice(1, 6).with_modifier(3), DamageType::Piercing, 3),
                ranged("Shortbow", 5, (80, 320), DiceExpr::dice(1, 6).with_modifier(3), DamageType::Piercing, 3),
            ],
            notes: vec!["Immune to Exhaustion".into(), "Languages: understands Common plus one other language but can't speak".into()],
            source: "srd-5.2.1".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_blocks_are_internally_consistent() {
        for m in srd_bestiary() {
            // Average of hit dice (rounded down) equals printed HP in 2024 stat blocks.
            assert_eq!(m.hit_dice.average().floor() as i32, m.hit_points, "{}", m.name);
            assert_eq!(m.initiative_bonus, m.scores.modifier(crate::Ability::Dexterity), "{}", m.name);
            for a in &m.attacks {
                // Printed attack bonus = ability mod + PB.
                assert_eq!(a.to_hit, a.ability_mod + m.proficiency_bonus, "{} {}", m.name, a.name);
            }
        }
    }
}
