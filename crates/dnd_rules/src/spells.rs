//! Spells from SRD 5.2.1 (`content/srd_spells.json`, extracted by `tools/extract_spells.py`) and the
//! math of casting them: damage/healing dice with upcasting and cantrip scaling, areas, targets.
//!
//! Automation levels (field `auto.level`):
//! * `Full` — every rules effect of the spell is implemented and covered by tests;
//! * `Partial` — attack/save/damage/healing core is automated, the rest of the text is DM-adjudicated;
//! * `Manual` — the engine handles slots, action economy, concentration, range and shows the text.

use crate::ability::Ability;
use crate::conditions::Condition;
use crate::damage::DamageType;
use crate::dice::DiceExpr;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AutoLevel {
    Full,
    Partial,
    #[default]
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpellAttackKind {
    Melee,
    Ranged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AreaShape {
    Sphere,
    Cube,
    Cone,
    Line,
    Emanation,
    Cylinder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Area {
    pub size_ft: u32,
    pub shape: AreaShape,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageSpec {
    pub dice: String,
    #[serde(rename = "type")]
    pub damage_type: DamageType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealSpec {
    pub dice: String,
    #[serde(default)]
    pub add_mod: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LightSpec {
    pub bright_ft: u32,
    pub dim_ft: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Secondary {
    pub save: Ability,
    pub area: Area,
    pub damage: DamageSpec,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct SpellAuto {
    pub level: AutoLevel,
    pub attack: Option<SpellAttackKind>,
    pub save: Option<Ability>,
    pub damage: Option<DamageSpec>,
    pub half_on_success: bool,
    pub healing: Option<HealSpec>,
    pub area: Option<Area>,
    pub condition: Option<Condition>,
    pub repeat_save: bool,
    pub upcast_dice: Option<String>,
    pub upcast_targets: Option<u8>,
    pub cantrip_scaling: Option<String>,
    pub light: Option<LightSpec>,
    pub darts: Option<u8>,
    pub rays: Option<u8>,
    pub beams_by_level: Option<Vec<(u8, u8)>>,
    pub buff: Option<String>,
    pub targets: Option<u8>,
    pub reaction: Option<String>,
    pub humanoid_only: bool,
    pub smite: bool,
    pub extra_vs: Vec<String>,
    pub extra_dice: Option<String>,
    pub grants_advantage_next_attack: bool,
    pub speed_reduction_ft: Option<u32>,
    pub no_reactions: bool,
    pub no_healing: bool,
    pub push_ft: Option<u32>,
    pub auto_hit: bool,
    pub secondary: Option<Secondary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spell {
    pub id: String,
    pub name: String,
    pub level: u8,
    pub school: String,
    pub classes: Vec<String>,
    pub casting_time: String,
    pub range: String,
    pub components: String,
    pub duration: String,
    pub ritual: bool,
    pub concentration: bool,
    pub verbal: bool,
    pub somatic: bool,
    pub material: Option<String>,
    pub description: String,
    #[serde(default)]
    pub auto: SpellAuto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CastingKind {
    Action,
    BonusAction,
    Reaction,
    /// One minute or longer: only outside combat.
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpellRange {
    SelfOnly,
    Touch,
    Feet(u32),
    Sight,
    Unlimited,
}

impl Spell {
    pub fn casting_kind(&self) -> CastingKind {
        let c = self.casting_time.as_str();
        if c.starts_with("Bonus Action") {
            CastingKind::BonusAction
        } else if c.starts_with("Reaction") {
            CastingKind::Reaction
        } else if c.starts_with("Action") {
            CastingKind::Action
        } else {
            CastingKind::Long
        }
    }

    pub fn range(&self) -> SpellRange {
        let r = self.range.as_str();
        if r.starts_with("Self") {
            SpellRange::SelfOnly
        } else if r.starts_with("Touch") {
            SpellRange::Touch
        } else if r.starts_with("Sight") {
            SpellRange::Sight
        } else if let Some(n) = r.strip_suffix(" feet").and_then(|n| n.replace(',', "").parse().ok()) {
            SpellRange::Feet(n)
        } else if let Some(n) = r.strip_suffix(" miles").or(r.strip_suffix(" mile")).and_then(|n| n.replace(',', "").parse::<u32>().ok()) {
            SpellRange::Feet(n * 5280)
        } else {
            SpellRange::Unlimited
        }
    }

    /// Reach of the spell in feet (Touch = 5 ft; Self = 0).
    pub fn range_ft(&self) -> u32 {
        match self.range() {
            SpellRange::SelfOnly => 0,
            SpellRange::Touch => 5,
            SpellRange::Feet(n) => n,
            SpellRange::Sight | SpellRange::Unlimited => u32::MAX,
        }
    }

    /// Duration in combat rounds (6 seconds each). `None` = instantaneous; `Some(u32::MAX)` = until dispelled.
    pub fn duration_rounds(&self) -> Option<u32> {
        let d = self.duration.trim_start_matches("Concentration, up to ").to_lowercase();
        if d.starts_with("instantaneous") {
            return None;
        }
        let n: u32 = d.split_whitespace().next().and_then(|n| n.parse().ok()).unwrap_or(1);
        Some(if d.contains("round") {
            n
        } else if d.contains("minute") {
            n * 10
        } else if d.contains("hour") {
            n * 600
        } else if d.contains("day") {
            n * 14_400
        } else {
            u32::MAX
        })
    }

    pub fn is_cantrip(&self) -> bool {
        self.level == 0
    }

    /// Damage dice for a cast at `slot_level` by a character of `caster_level` (cantrip upgrades at 5/11/17).
    pub fn damage_dice(&self, slot_level: u8, caster_level: u8) -> Option<DiceExpr> {
        let base = DiceExpr::parse(&self.auto.damage.as_ref()?.dice).ok()?;
        Some(self.scaled(base, slot_level, caster_level))
    }

    pub fn healing_dice(&self, slot_level: u8, caster_level: u8) -> Option<DiceExpr> {
        let base = DiceExpr::parse(&self.auto.healing.as_ref()?.dice).ok()?;
        Some(self.scaled(base, slot_level, caster_level))
    }

    fn scaled(&self, mut base: DiceExpr, slot_level: u8, caster_level: u8) -> DiceExpr {
        if self.is_cantrip() {
            if let Some(step) = self.auto.cantrip_scaling.as_deref().and_then(|s| DiceExpr::parse(s).ok()) {
                let tiers = cantrip_tier(caster_level) - 1;
                for _ in 0..tiers {
                    add_dice(&mut base, &step);
                }
            }
        } else if let Some(step) = self.auto.upcast_dice.as_deref().and_then(|s| DiceExpr::parse(s).ok()) {
            for _ in self.level..slot_level {
                add_dice(&mut base, &step);
            }
        }
        base
    }

    /// Number of separate attack rolls / darts.
    pub fn projectiles(&self, slot_level: u8, caster_level: u8) -> u8 {
        if let Some(d) = self.auto.darts {
            return d + slot_level.saturating_sub(self.level);
        }
        if let Some(r) = self.auto.rays {
            return r + slot_level.saturating_sub(self.level);
        }
        if let Some(b) = &self.auto.beams_by_level {
            return b.iter().filter(|(lvl, _)| caster_level >= *lvl).map(|(_, n)| *n).max().unwrap_or(1);
        }
        1
    }

    /// Maximum number of chosen targets (for single-target or "up to N creatures" spells).
    pub fn max_targets(&self, slot_level: u8) -> u8 {
        let base = self.auto.targets.unwrap_or(1);
        base + self.auto.upcast_targets.unwrap_or(0) * slot_level.saturating_sub(self.level)
    }
}

/// Cantrip damage tier: 1 below level 5, 2 at 5–10, 3 at 11–16, 4 at 17+.
pub fn cantrip_tier(level: u8) -> u8 {
    match level {
        0..=4 => 1,
        5..=10 => 2,
        11..=16 => 3,
        _ => 4,
    }
}

fn add_dice(base: &mut DiceExpr, step: &DiceExpr) {
    for t in &step.terms {
        if let Some(existing) = base.terms.iter_mut().find(|b| b.sides == t.sides && b.negative == t.negative) {
            existing.count += t.count;
        } else {
            base.terms.push(*t);
        }
    }
}

static SPELLS: OnceLock<Vec<Spell>> = OnceLock::new();

/// All SRD 5.2.1 spells.
pub fn all() -> &'static [Spell] {
    SPELLS.get_or_init(|| serde_json::from_str(include_str!("../../../content/srd_spells.json")).expect("valid spell data"))
}

pub fn spell(id: &str) -> Option<&'static Spell> {
    all().iter().find(|s| s.id == id)
}

/// Spells on a class's list (e.g. "Wizard").
pub fn class_list(class: &str) -> impl Iterator<Item = &'static Spell> + '_ {
    all().iter().filter(move |s| s.classes.iter().any(|c| c == class))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_srd_spells_load() {
        assert_eq!(all().len(), 339);
        assert_eq!(all().iter().filter(|s| s.level == 0).count(), 27);
        let names: std::collections::HashSet<_> = all().iter().map(|s| s.id.as_str()).collect();
        assert_eq!(names.len(), 339, "ids are unique");
    }

    #[test]
    fn fireball_matches_srd() {
        // "Each creature in a 20-foot-radius Sphere ... Dexterity saving throw, taking 8d6 Fire damage on a
        //  failed save or half as much damage on a successful one ... increases by 1d6 for each slot level above 3."
        let f = spell("fireball").unwrap();
        assert_eq!((f.level, f.range_ft(), f.casting_kind()), (3, 150, CastingKind::Action));
        assert_eq!(f.auto.save, Some(Ability::Dexterity));
        assert!(f.auto.half_on_success);
        assert_eq!(f.auto.area, Some(Area { size_ft: 20, shape: AreaShape::Sphere }));
        assert_eq!(f.damage_dice(3, 5).unwrap().to_string(), "8d6");
        assert_eq!(f.damage_dice(5, 9).unwrap().to_string(), "10d6");
        assert_eq!(f.auto.level, AutoLevel::Full);
    }

    #[test]
    fn cantrips_scale_with_character_level() {
        let fb = spell("fire_bolt").unwrap();
        assert_eq!(fb.damage_dice(0, 1).unwrap().to_string(), "1d10");
        assert_eq!(fb.damage_dice(0, 5).unwrap().to_string(), "2d10");
        assert_eq!(fb.damage_dice(0, 11).unwrap().to_string(), "3d10");
        assert_eq!(fb.damage_dice(0, 17).unwrap().to_string(), "4d10");
        let eb = spell("eldritch_blast").unwrap();
        assert_eq!((eb.projectiles(0, 4), eb.projectiles(0, 5), eb.projectiles(0, 17)), (1, 2, 4));
        assert_eq!(eb.damage_dice(0, 17).unwrap().to_string(), "1d10", "Eldritch Blast scales by beams, not dice");
    }

    #[test]
    fn healing_and_projectiles_upcast() {
        let cw = spell("cure_wounds").unwrap();
        assert_eq!(cw.healing_dice(1, 1).unwrap().to_string(), "2d8");
        assert_eq!(cw.healing_dice(3, 5).unwrap().to_string(), "6d8");
        let mm = spell("magic_missile").unwrap();
        assert_eq!((mm.projectiles(1, 1), mm.projectiles(3, 5)), (3, 5));
        let sr = spell("scorching_ray").unwrap();
        assert_eq!(sr.projectiles(3, 5), 4);
        let hp = spell("hold_person").unwrap();
        assert_eq!((hp.max_targets(2), hp.max_targets(4)), (1, 3));
        assert_eq!(hp.auto.condition, Some(Condition::Paralyzed));
        assert!(hp.auto.repeat_save && hp.concentration);
        assert_eq!(hp.duration_rounds(), Some(10));
    }

    #[test]
    fn light_spell_matches_srd() {
        // "sheds Bright Light in a 20-foot radius and Dim Light for an additional 20 feet", 1 hour.
        let l = spell("light").unwrap();
        assert_eq!(l.auto.light, Some(LightSpec { bright_ft: 20, dim_ft: 20 }));
        assert_eq!(l.range(), SpellRange::Touch);
        assert_eq!(l.duration_rounds(), Some(600));
        assert!(!l.concentration);
    }

    #[test]
    fn revised_2024_versions_are_used() {
        // 2024 Inflict Wounds is a Constitution save (2014 was a melee spell attack).
        let iw = spell("inflict_wounds").unwrap();
        assert_eq!(iw.auto.save, Some(Ability::Constitution));
        assert!(iw.auto.attack.is_none());
        // 2024 Poison Spray is a ranged spell attack for 1d12.
        let ps = spell("poison_spray").unwrap();
        assert_eq!(ps.auto.attack, Some(SpellAttackKind::Ranged));
        assert_eq!(ps.auto.damage.as_ref().unwrap().dice, "1d12");
        // 2024 Divine Smite is a Bonus Action spell.
        assert_eq!(spell("divine_smite").unwrap().casting_kind(), CastingKind::BonusAction);
    }

    #[test]
    fn class_lists() {
        assert!(class_list("Wizard").any(|s| s.id == "fireball"));
        assert!(!class_list("Cleric").any(|s| s.id == "fireball"));
        assert!(class_list("Cleric").any(|s| s.id == "cure_wounds"));
    }
}
