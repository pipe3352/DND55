//! Player character sheets: 2024 character creation (Steps: class, origin = background + species,
//! ability scores, background ability adjustments) and derived statistics.
//! (SRD 5.2.1 "Character Creation", "Character Origins", "Classes")

use crate::ability::{Ability, AbilityScores, Skill, proficiency_bonus};
use crate::attack::{AttackProfile, DamagePart};
use crate::content::{self, BackgroundId, ClassId, OriginFeat, Size, SpeciesId};
use crate::damage::DamageType;
use crate::dice::DiceExpr;
use crate::equipment::{self, ArmorCategory, WeaponKind, WeaponProperty};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreMethod {
    StandardArray,
    PointBuy,
    /// 4d6-drop-lowest rolled by the authoritative host; the host records the rolls in the session log.
    Rolled,
    /// Entered manually; requires DM approval and is flagged as a rules override.
    ManualDmApproved,
}

/// "Your background lists three abilities; increase one of those scores by 2 and a different one by 1,
/// or increase all three by 1. None of these increases can raise a score above 20."
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundBonus {
    PlusTwoPlusOne { plus_two: Ability, plus_one: Ability },
    ThreePlusOnes,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterSheet {
    pub name: String,
    pub species: SpeciesId,
    pub size: Size,
    pub class: ClassId,
    pub level: u8,
    pub background: BackgroundId,
    pub score_method: ScoreMethod,
    /// Scores before the background adjustment.
    pub base_scores: AbilityScores,
    pub background_bonus: BackgroundBonus,
    pub class_skills: Vec<Skill>,
    /// Human "Skillful" grants one extra skill.
    pub species_skill: Option<Skill>,
    pub body_armor: Option<String>,
    pub shield: bool,
    pub weapons: Vec<String>,
    pub weapon_masteries: Vec<String>,
    pub experience_points: u32,
    pub biography: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Issue {
    pub severity: Severity,
    /// Stable machine key (localizable), e.g. `char.scores.standard_array`.
    pub key: String,
    pub detail: String,
}

fn err(key: &str, detail: impl Into<String>) -> Issue {
    Issue { severity: Severity::Error, key: key.into(), detail: detail.into() }
}
fn warn(key: &str, detail: impl Into<String>) -> Issue {
    Issue { severity: Severity::Warning, key: key.into(), detail: detail.into() }
}

/// SRD "Ability Score Point Costs" table (8..=15).
pub fn point_cost(score: u8) -> Option<u32> {
    Some(match score {
        8 => 0,
        9 => 1,
        10 => 2,
        11 => 3,
        12 => 4,
        13 => 5,
        14 => 7,
        15 => 9,
        _ => return None,
    })
}

pub const STANDARD_ARRAY: [u8; 6] = [15, 14, 13, 12, 10, 8];
pub const POINT_BUY_BUDGET: u32 = 27;

impl CharacterSheet {
    pub fn class_def(&self) -> &'static content::ClassDef {
        content::class(self.class)
    }
    pub fn species_def(&self) -> &'static content::SpeciesDef {
        content::species(self.species)
    }
    pub fn background_def(&self) -> &'static content::BackgroundDef {
        content::background(self.background)
    }

    /// Final ability scores after the background adjustment.
    pub fn scores(&self) -> AbilityScores {
        let mut s = self.base_scores;
        let bg = self.background_def();
        match self.background_bonus {
            BackgroundBonus::PlusTwoPlusOne { plus_two, plus_one } => {
                s.set(plus_two, (s.get(plus_two) + 2).min(20));
                s.set(plus_one, (s.get(plus_one) + 1).min(20));
            }
            BackgroundBonus::ThreePlusOnes => {
                for a in bg.abilities {
                    s.set(a, (s.get(a) + 1).min(20));
                }
            }
        }
        s
    }

    pub fn modifier(&self, a: Ability) -> i32 {
        self.scores().modifier(a)
    }
    pub fn proficiency_bonus(&self) -> i32 {
        proficiency_bonus(self.level)
    }

    /// Level 1: hit die maximum + Con modifier ("Level 1 Hit Points by Class"). Later levels use the
    /// fixed average (die/2 + 1) + Con modifier per level; rolled HP is a pending option.
    /// Dwarven Toughness adds 1 per level. Minimum 1 HP per level.
    pub fn max_hp(&self) -> i32 {
        let die = self.class_def().hit_die as i32;
        let con = self.modifier(Ability::Constitution);
        let bonus = self.species_def().hp_per_level_bonus;
        let first = (die + con).max(1) + bonus;
        let later = (1..self.level as i32).map(|_| (die / 2 + 1 + con).max(1) + bonus).sum::<i32>();
        first + later
    }

    pub fn armor_class(&self) -> i32 {
        let body = self.body_armor.as_deref().and_then(equipment::armor);
        equipment::armor_class(body, self.shield, self.modifier(Ability::Dexterity))
    }

    pub fn has_feat(&self, f: OriginFeat) -> bool {
        self.background_def().feat == f
    }

    /// Initiative is a Dexterity check; Alert adds the Proficiency Bonus.
    pub fn initiative_bonus(&self) -> i32 {
        let mut b = self.modifier(Ability::Dexterity);
        if self.has_feat(OriginFeat::Alert) {
            b += self.proficiency_bonus();
        }
        b
    }

    pub fn speed_ft(&self) -> i32 {
        self.species_def().speed_ft
    }
    pub fn darkvision_ft(&self) -> u32 {
        self.species_def().darkvision_ft
    }

    pub fn skill_proficient(&self, s: Skill) -> bool {
        self.class_skills.contains(&s) || self.background_def().skills.contains(&s) || self.species_skill == Some(s)
    }
    pub fn skill_bonus(&self, s: Skill) -> i32 {
        self.modifier(s.ability()) + if self.skill_proficient(s) { self.proficiency_bonus() } else { 0 }
    }
    pub fn save_bonus(&self, a: Ability) -> i32 {
        self.modifier(a) + if self.class_def().saving_throws.contains(&a) { self.proficiency_bonus() } else { 0 }
    }
    pub fn passive_perception(&self) -> i32 {
        10 + self.skill_bonus(Skill::Perception)
    }

    /// All attacks the character can make: one per carried weapon (plus a thrown variant), and Unarmed Strike.
    pub fn attacks(&self) -> Vec<AttackProfile> {
        let mut out = Vec::new();
        let pb = self.proficiency_bonus();
        let str_m = self.modifier(Ability::Strength);
        let dex_m = self.modifier(Ability::Dexterity);
        for id in &self.weapons {
            let Some(w) = equipment::weapon(id) else { continue };
            let proficient = self.class_def().proficient_with(w);
            let mastery = if self.weapon_masteries.iter().any(|m| m == id) && proficient { Some(w.mastery) } else { None };
            // Melee uses Strength, ranged uses Dexterity; Finesse lets you choose (we take the better).
            let melee_mod = if w.has(WeaponProperty::Finesse) { str_m.max(dex_m) } else { str_m };
            let (ability_mod, reach, range) = match w.kind {
                WeaponKind::Melee => (melee_mod, w.reach_ft(), None),
                WeaponKind::Ranged => (dex_m, 0, w.range),
            };
            let to_hit = ability_mod + if proficient { pb } else { 0 };
            let dmg = vec![DamagePart { dice: w.damage_dice().with_modifier(ability_mod), damage_type: w.damage_type }];
            out.push(AttackProfile {
                name: w.name.to_string(),
                kind: w.kind,
                to_hit,
                damage: dmg.clone(),
                extra_damage_with_advantage: vec![],
                reach_ft: reach,
                range_ft: range,
                mastery,
                ability_mod,
            });
            // Thrown melee weapons can make a ranged attack using the same ability modifier.
            if w.kind == WeaponKind::Melee && w.has(WeaponProperty::Thrown) {
                out.push(AttackProfile {
                    name: format!("{} (thrown)", w.name),
                    kind: WeaponKind::Ranged,
                    to_hit,
                    damage: dmg,
                    extra_damage_with_advantage: vec![],
                    reach_ft: 0,
                    range_ft: w.range,
                    mastery,
                    ability_mod,
                });
            }
        }
        // Unarmed Strike (Damage option): Str mod + PB to hit; 1 + Str mod Bludgeoning.
        out.push(AttackProfile {
            name: "Unarmed Strike".into(),
            kind: WeaponKind::Melee,
            to_hit: str_m + pb,
            damage: vec![DamagePart { dice: DiceExpr::default().with_modifier(1 + str_m), damage_type: DamageType::Bludgeoning }],
            extra_damage_with_advantage: vec![],
            reach_ft: 5,
            range_ft: None,
            mastery: None,
            ability_mod: str_m,
        });
        out
    }

    /// Validates the sheet against the 2024 creation rules. Errors block creation; warnings are shown.
    pub fn validate(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        let name = self.name.trim();
        if name.is_empty() || name.chars().count() > 40 {
            issues.push(err("char.name", "Name must be 1–40 characters"));
        }
        if !(1..=20).contains(&self.level) {
            issues.push(err("char.level", "Level must be 1–20"));
        }
        if self.level > 1 {
            issues.push(warn("char.level.features", "Class features above level 1 are not yet automated"));
        }
        if !self.species_def().sizes.contains(&self.size) {
            issues.push(err("char.size", format!("{} can't be {:?}", self.species_def().name, self.size)));
        }

        // Ability scores.
        let base = self.base_scores.0;
        match self.score_method {
            ScoreMethod::StandardArray => {
                let mut sorted = base;
                sorted.sort_unstable_by(|a, b| b.cmp(a));
                if sorted != STANDARD_ARRAY {
                    issues.push(err("char.scores.standard_array", "Scores must be 15, 14, 13, 12, 10, 8 in any order"));
                }
            }
            ScoreMethod::PointBuy => match base.iter().map(|s| point_cost(*s)).collect::<Option<Vec<_>>>() {
                None => issues.push(err("char.scores.point_buy_range", "Point-buy scores must be between 8 and 15")),
                Some(costs) => {
                    let total: u32 = costs.iter().sum();
                    if total > POINT_BUY_BUDGET {
                        issues.push(err("char.scores.point_buy_budget", format!("Spent {total} of {POINT_BUY_BUDGET} points")));
                    }
                }
            },
            ScoreMethod::Rolled => {
                if base.iter().any(|s| !(3..=18).contains(s)) {
                    issues.push(err("char.scores.rolled_range", "Rolled scores must be 3–18"));
                }
            }
            ScoreMethod::ManualDmApproved => {
                issues.push(warn("char.scores.manual", "Manually entered scores (DM override)"));
            }
        }

        // Background adjustment.
        let bg = self.background_def();
        if let BackgroundBonus::PlusTwoPlusOne { plus_two, plus_one } = self.background_bonus {
            if plus_two == plus_one {
                issues.push(err("char.background.same_ability", "+2 and +1 must go to different abilities"));
            }
            if !bg.abilities.contains(&plus_two) || !bg.abilities.contains(&plus_one) {
                issues.push(err("char.background.ability", format!("{} only allows {:?}", bg.name, bg.abilities)));
            }
        }
        let raw_after: Vec<u8> = match self.background_bonus {
            BackgroundBonus::PlusTwoPlusOne { plus_two, plus_one } => Ability::ALL
                .iter()
                .map(|a| base[a.index()] + if *a == plus_two { 2 } else { 0 } + if *a == plus_one { 1 } else { 0 })
                .collect(),
            BackgroundBonus::ThreePlusOnes => {
                Ability::ALL.iter().map(|a| base[a.index()] + u8::from(bg.abilities.contains(a))).collect()
            }
        };
        if raw_after.iter().any(|s| *s > 20) {
            issues.push(err("char.scores.max20", "Background increases can't raise a score above 20"));
        }

        // Skills.
        let cd = self.class_def();
        let mut uniq = self.class_skills.clone();
        uniq.sort();
        uniq.dedup();
        if uniq.len() != self.class_skills.len() {
            issues.push(err("char.skills.duplicate", "Duplicate class skill"));
        }
        if self.class_skills.len() != cd.skill_choices as usize {
            issues.push(err("char.skills.count", format!("{} chooses {} skills", cd.name, cd.skill_choices)));
        }
        if !cd.skill_list.is_empty() {
            for s in &self.class_skills {
                if !cd.skill_list.contains(s) {
                    issues.push(err("char.skills.not_in_list", format!("{s:?} isn't a {} skill option", cd.name)));
                }
            }
        }
        for s in &self.class_skills {
            if bg.skills.contains(s) {
                issues.push(warn("char.skills.overlap", format!("{s:?} is already granted by the background; choose another")));
            }
        }
        match (self.species, self.species_skill) {
            (SpeciesId::Human, None) => issues.push(err("char.species.skillful", "Human Skillful: choose one skill")),
            (SpeciesId::Human, Some(_)) => {}
            (_, Some(_)) => issues.push(err("char.species.extra_skill", "Only Humans gain the Skillful skill")),
            _ => {}
        }

        // Equipment.
        if let Some(id) = &self.body_armor {
            match equipment::armor(id) {
                None => issues.push(err("char.armor.unknown", format!("Unknown armor {id}"))),
                Some(a) if a.category == ArmorCategory::Shield => issues.push(err("char.armor.shield_as_body", "Use the shield flag")),
                Some(a) => {
                    if !cd.trained_in_armor(a.category) {
                        issues.push(warn("char.armor.untrained", format!("No training with {:?} armor: Disadvantage on Str/Dex D20 Tests and no spellcasting", a.category)));
                    }
                    if let Some(req) = a.strength_req {
                        if self.scores().get(Ability::Strength) < req {
                            issues.push(warn("char.armor.strength", format!("{} needs Str {req}: Speed reduced by 10 ft", a.name)));
                        }
                    }
                }
            }
        }
        if self.shield && !cd.trained_in_armor(ArmorCategory::Shield) {
            issues.push(warn("char.armor.shield_untrained", "No Shield training"));
        }
        for w in &self.weapons {
            if equipment::weapon(w).is_none() {
                issues.push(err("char.weapon.unknown", format!("Unknown weapon {w}")));
            }
        }
        if self.weapon_masteries.len() > cd.weapon_mastery_count as usize {
            issues.push(err("char.mastery.count", format!("{} masters {} weapon kinds at level 1", cd.name, cd.weapon_mastery_count)));
        }
        for m in &self.weapon_masteries {
            match equipment::weapon(m) {
                None => issues.push(err("char.mastery.unknown", format!("Unknown weapon {m}"))),
                Some(w) => {
                    if !cd.proficient_with(w) {
                        issues.push(err("char.mastery.not_proficient", format!("Not proficient with {}", w.name)));
                    }
                    if self.class == ClassId::Barbarian && w.kind != WeaponKind::Melee {
                        issues.push(err("char.mastery.barbarian_melee", "Barbarian masteries must be melee weapons"));
                    }
                }
            }
        }
        issues
    }

    pub fn is_valid(&self) -> bool {
        self.validate().iter().all(|i| i.severity != Severity::Error)
    }

    /// A ready-made level 1 Fighter used by tests, the demo campaign and quick-start.
    pub fn example_fighter(name: &str) -> Self {
        CharacterSheet {
            name: name.into(),
            species: SpeciesId::Dwarf,
            size: Size::Medium,
            class: ClassId::Fighter,
            level: 1,
            background: BackgroundId::Soldier,
            score_method: ScoreMethod::StandardArray,
            base_scores: AbilityScores::new(15, 13, 14, 8, 12, 10),
            background_bonus: BackgroundBonus::PlusTwoPlusOne { plus_two: Ability::Strength, plus_one: Ability::Constitution },
            class_skills: vec![Skill::Perception, Skill::Survival],
            species_skill: None,
            body_armor: Some("chain_mail".into()),
            shield: true,
            weapons: vec!["longsword".into(), "javelin".into(), "greatsword".into()],
            weapon_masteries: vec!["longsword".into(), "greatsword".into(), "javelin".into()],
            experience_points: 0,
            biography: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::Mastery;

    #[test]
    fn example_fighter_derived_stats() {
        let c = CharacterSheet::example_fighter("Brunhild");
        assert!(c.is_valid(), "{:?}", c.validate());
        let s = c.scores();
        assert_eq!((s.get(Ability::Strength), s.get(Ability::Constitution)), (17, 15));
        // Fighter: 10 + Con mod (+2) + Dwarven Toughness (+1)
        assert_eq!(c.max_hp(), 13);
        assert_eq!(c.armor_class(), 18); // Chain Mail 16 + Shield 2
        assert_eq!(c.save_bonus(Ability::Strength), 3 + 2);
        assert_eq!(c.save_bonus(Ability::Wisdom), 1);
        assert_eq!(c.skill_bonus(Skill::Athletics), 3 + 2, "Soldier grants Athletics");
        assert_eq!(c.darkvision_ft(), 120);
        let ls = c.attacks().into_iter().find(|a| a.name == "Longsword").unwrap();
        assert_eq!(ls.to_hit, 5);
        assert_eq!(ls.damage[0].dice.to_string(), "1d8+3");
        assert_eq!(ls.mastery, Some(Mastery::Sap));
        let unarmed = c.attacks().into_iter().find(|a| a.name == "Unarmed Strike").unwrap();
        assert_eq!(unarmed.damage[0].dice.to_string(), "4");
        assert!(c.attacks().iter().any(|a| a.name == "Javelin (thrown)" && a.range_ft == Some((30, 120))));
    }

    #[test]
    fn rejects_invalid_scores_and_background_choices() {
        let mut c = CharacterSheet::example_fighter("X");
        c.base_scores = AbilityScores::new(15, 15, 13, 12, 10, 8);
        assert!(!c.is_valid());

        let mut c = CharacterSheet::example_fighter("X");
        c.background_bonus = BackgroundBonus::PlusTwoPlusOne { plus_two: Ability::Charisma, plus_one: Ability::Strength };
        assert!(!c.is_valid(), "Soldier can't raise Charisma");

        let mut c = CharacterSheet::example_fighter("X");
        c.score_method = ScoreMethod::PointBuy;
        c.base_scores = AbilityScores::new(15, 15, 15, 8, 8, 8); // 27 points exactly
        assert!(c.is_valid(), "{:?}", c.validate());
        c.base_scores = AbilityScores::new(15, 15, 15, 9, 8, 8); // 28
        assert!(!c.is_valid());
    }

    #[test]
    fn alert_adds_proficiency_to_initiative() {
        let mut c = CharacterSheet::example_fighter("X");
        c.background = BackgroundId::Criminal;
        c.background_bonus = BackgroundBonus::ThreePlusOnes;
        // Dex 13 + 1 = 14 -> +2, plus PB 2
        assert_eq!(c.initiative_bonus(), 4);
    }

    #[test]
    fn level_one_hp_by_class() {
        for (class, expected) in [(ClassId::Barbarian, 12), (ClassId::Ranger, 10), (ClassId::Rogue, 8), (ClassId::Wizard, 6)] {
            let mut c = CharacterSheet::example_fighter("X");
            c.species = SpeciesId::Elf;
            c.class = class;
            c.base_scores = AbilityScores::new(15, 13, 9, 8, 12, 10); // Con 9 + 1 = 10 -> +0
            assert_eq!(c.max_hp(), expected, "{class:?}");
        }
    }

    #[test]
    fn mastery_limits() {
        let mut c = CharacterSheet::example_fighter("X");
        c.weapon_masteries.push("dagger".into());
        assert!(!c.is_valid(), "Fighter masters only 3 at level 1");
        let mut c = CharacterSheet::example_fighter("X");
        c.class = ClassId::Wizard;
        c.class_skills = vec![Skill::Arcana, Skill::History];
        assert!(!c.is_valid(), "Wizards have no Weapon Mastery");
    }
}
