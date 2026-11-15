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
    PlusTwoPlusOne {
        plus_two: Ability,
        plus_one: Ability,
    },
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
    /// Subclass name (the SRD has one per class), chosen at level 3.
    #[serde(default)]
    pub subclass: Option<String>,
    /// Ability Score Improvement feat increases taken at ASI levels: (ability, +1 or +2).
    #[serde(default)]
    pub asi: Vec<(Ability, u8)>,
    #[serde(default)]
    pub fighting_style: Option<FightingStyle>,
    #[serde(default)]
    pub expertise: Vec<Skill>,
    #[serde(default)]
    pub cantrips: Vec<String>,
    #[serde(default)]
    pub prepared_spells: Vec<String>,
    #[serde(default)]
    pub inventory: Vec<crate::items::ItemStack>,
    #[serde(default)]
    pub currency: crate::items::Currency,
}

/// SRD 5.2.1 Fighting Style feats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FightingStyle {
    /// +2 to attack rolls with Ranged weapons.
    Archery,
    /// +1 AC while wearing Light, Medium or Heavy armor.
    Defense,
    /// Treat 1s and 2s on damage dice of two-handed melee weapons as 3s.
    GreatWeaponFighting,
    /// Add the ability modifier to the Light-property extra attack (not yet automated).
    TwoWeaponFighting,
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
    Issue {
        severity: Severity::Error,
        key: key.into(),
        detail: detail.into(),
    }
}
fn warn(key: &str, detail: impl Into<String>) -> Issue {
    Issue {
        severity: Severity::Warning,
        key: key.into(),
        detail: detail.into(),
    }
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
        // Ability Score Improvement feat: "can't increase an ability score above 20".
        for (a, n) in &self.asi {
            s.set(*a, (s.get(*a) + n).min(20));
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
        let later = (1..self.level as i32)
            .map(|_| (die / 2 + 1 + con).max(1) + bonus)
            .sum::<i32>();
        // Draconic Resilience (Sorcerer 3): +3, then +1 per further Sorcerer level.
        let draconic = if self.has_class_feature("Draconic Resilience") {
            self.level as i32
        } else {
            0
        };
        first + later + draconic
    }

    pub fn subclass_name(&self) -> Option<&str> {
        self.subclass.as_deref()
    }

    /// Whether the character has a class or subclass feature (by SRD name prefix) at its level.
    pub fn has_class_feature(&self, name: &str) -> bool {
        crate::classes::has_feature(self.class, self.level, self.subclass_name(), name)
    }

    fn wearing_armor(&self) -> bool {
        self.body_armor
            .as_deref()
            .and_then(equipment::armor)
            .is_some()
    }

    pub fn armor_class(&self) -> i32 {
        let body = self.body_armor.as_deref().and_then(equipment::armor);
        let dex = self.modifier(Ability::Dexterity);
        let mut ac = equipment::armor_class(body, self.shield, dex);
        if body.is_none() {
            let shield = if self.shield { 2 } else { 0 };
            // Unarmored Defense (Barbarian): 10 + Dex + Con, shield allowed.
            if self.class == ClassId::Barbarian {
                ac = ac.max(10 + dex + self.modifier(Ability::Constitution) + shield);
            }
            // Unarmored Defense (Monk): 10 + Dex + Wis, no armor and no shield.
            if self.class == ClassId::Monk && !self.shield {
                ac = ac.max(10 + dex + self.modifier(Ability::Wisdom));
            }
            // Draconic Resilience: base AC 10 + Dex + Cha while not wearing armor.
            if self.has_class_feature("Draconic Resilience") {
                ac = ac.max(10 + dex + self.modifier(Ability::Charisma) + shield);
            }
        } else if self.fighting_style == Some(FightingStyle::Defense) {
            ac += 1;
        }
        ac
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
        let mut speed = self.species_def().speed_ft;
        let heavy = self
            .body_armor
            .as_deref()
            .and_then(equipment::armor)
            .is_some_and(|a| a.category == ArmorCategory::Heavy);
        if self.class == ClassId::Barbarian && self.has_class_feature("Fast Movement") && !heavy {
            speed += 10;
        }
        if self.class == ClassId::Monk && !self.wearing_armor() && !self.shield {
            speed += crate::classes::unarmored_movement(self.level);
        }
        speed
    }
    pub fn darkvision_ft(&self) -> u32 {
        self.species_def().darkvision_ft
    }

    pub fn skill_proficient(&self, s: Skill) -> bool {
        self.class_skills.contains(&s)
            || self.background_def().skills.contains(&s)
            || self.species_skill == Some(s)
    }
    pub fn skill_bonus(&self, s: Skill) -> i32 {
        let pb = self.proficiency_bonus();
        let prof = if self.expertise.contains(&s) && self.skill_proficient(s) {
            2 * pb
        } else if self.skill_proficient(s) {
            pb
        } else if self.class == ClassId::Bard && self.level >= 2 {
            pb / 2 // Jack of All Trades: half PB (round down) to checks you aren't proficient in
        } else {
            0
        };
        self.modifier(s.ability()) + prof
    }

    // ---------------------------------------------------------------- spellcasting

    pub fn spellcasting_ability(&self) -> Option<Ability> {
        crate::classes::spellcasting(self.class).map(|(a, _)| a)
    }

    /// Spell save DC = 8 + spellcasting ability modifier + Proficiency Bonus.
    pub fn spell_save_dc(&self) -> Option<i32> {
        self.spellcasting_ability()
            .map(|a| 8 + self.modifier(a) + self.proficiency_bonus())
    }

    /// Spell attack bonus = spellcasting ability modifier + Proficiency Bonus.
    pub fn spell_attack_bonus(&self) -> Option<i32> {
        self.spellcasting_ability()
            .map(|a| self.modifier(a) + self.proficiency_bonus())
    }

    /// Prepared leveled spells including those always prepared by class/subclass features.
    pub fn all_prepared(&self) -> Vec<String> {
        let mut v = self.prepared_spells.clone();
        for s in crate::classes::subclass_always_prepared(self.class, self.level) {
            if crate::spells::spell(s).is_some() && !v.iter().any(|x| x == s) {
                v.push(s.to_string());
            }
        }
        v
    }

    pub fn knows_spell(&self, id: &str) -> bool {
        self.cantrips.iter().any(|c| c == id) || self.all_prepared().iter().any(|c| c == id)
    }

    pub fn spell_slots(&self) -> [u8; 9] {
        crate::classes::spell_slots(self.class, self.level)
    }

    pub fn attacks_per_action(&self) -> u8 {
        crate::classes::attacks_per_action(self.class, self.level)
    }

    /// Lowest natural d20 that scores a Critical Hit with weapons/unarmed (Champion).
    pub fn crit_threshold(&self) -> u32 {
        if self.has_class_feature("Superior Critical") {
            18
        } else if self.has_class_feature("Improved Critical") {
            19
        } else {
            20
        }
    }

    fn martial_arts_active(&self) -> bool {
        self.class == ClassId::Monk && !self.wearing_armor() && !self.shield
    }
    pub fn save_bonus(&self, a: Ability) -> i32 {
        self.modifier(a)
            + if self.class_def().saving_throws.contains(&a) {
                self.proficiency_bonus()
            } else {
                0
            }
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
            let Some(w) = equipment::weapon(id) else {
                continue;
            };
            let proficient = self.class_def().proficient_with(w);
            let mastery = if self.weapon_masteries.iter().any(|m| m == id) && proficient {
                Some(w.mastery)
            } else {
                None
            };
            // Melee uses Strength, ranged uses Dexterity; Finesse lets you choose (we take the better).
            let monk_weapon = self.martial_arts_active()
                && w.kind == WeaponKind::Melee
                && (w.category == equipment::WeaponCategory::Simple
                    || w.has(WeaponProperty::Light));
            let melee_mod = if w.has(WeaponProperty::Finesse) || monk_weapon {
                str_m.max(dex_m)
            } else {
                str_m
            };
            let (ability_mod, reach, range) = match w.kind {
                WeaponKind::Melee => (melee_mod, w.reach_ft(), None),
                WeaponKind::Ranged => (dex_m, 0, w.range),
            };
            let archery = if w.kind == WeaponKind::Ranged
                && self.fighting_style == Some(FightingStyle::Archery)
            {
                2
            } else {
                0
            };
            let to_hit = ability_mod + if proficient { pb } else { 0 } + archery;
            let mut dice = w.damage_dice();
            if monk_weapon {
                // Martial Arts die replaces the weapon's damage if larger.
                let ma = crate::classes::martial_arts_die(self.level);
                if dice.terms.len() == 1 && dice.terms[0].count == 1 && dice.terms[0].sides < ma {
                    dice = DiceExpr::dice(1, ma);
                }
            }
            let dmg = vec![DamagePart {
                dice: dice.with_modifier(ability_mod),
                damage_type: w.damage_type,
            }];
            let uses = if ability_mod == str_m && w.kind == WeaponKind::Melee {
                Ability::Strength
            } else {
                Ability::Dexterity
            };
            let meta = AttackProfile {
                ability: Some(uses),
                finesse_or_ranged: w.has(WeaponProperty::Finesse) || w.kind == WeaponKind::Ranged,
                weapon_id: Some(w.id.to_string()),
                two_handed_melee: w.kind == WeaponKind::Melee && w.has(WeaponProperty::TwoHanded),
                ..Default::default()
            };
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
                ..meta.clone()
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
                    ..AttackProfile {
                        finesse_or_ranged: true,
                        two_handed_melee: false,
                        ..meta
                    }
                });
            }
        }
        // Unarmed Strike (Damage option): Str mod + PB to hit; 1 + Str mod Bludgeoning.
        // Monk Martial Arts: Dex may replace Str, and the Martial Arts die replaces the 1.
        let (un_mod, un_dice, un_ability) = if self.martial_arts_active() {
            let m = str_m.max(dex_m);
            (
                m,
                DiceExpr::dice(1, crate::classes::martial_arts_die(self.level)).with_modifier(m),
                if dex_m > str_m {
                    Ability::Dexterity
                } else {
                    Ability::Strength
                },
            )
        } else {
            (
                str_m,
                DiceExpr::default().with_modifier(1 + str_m),
                Ability::Strength,
            )
        };
        out.push(AttackProfile {
            name: "Unarmed Strike".into(),
            kind: WeaponKind::Melee,
            to_hit: un_mod + pb,
            damage: vec![DamagePart {
                dice: un_dice,
                damage_type: DamageType::Bludgeoning,
            }],
            extra_damage_with_advantage: vec![],
            reach_ft: 5,
            range_ft: None,
            mastery: None,
            ability_mod: un_mod,
            ability: Some(un_ability),
            ..Default::default()
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
        self.validate_progression(&mut issues);
        if !self.species_def().sizes.contains(&self.size) {
            issues.push(err(
                "char.size",
                format!("{} can't be {:?}", self.species_def().name, self.size),
            ));
        }

        // Ability scores.
        let base = self.base_scores.0;
        match self.score_method {
            ScoreMethod::StandardArray => {
                let mut sorted = base;
                sorted.sort_unstable_by(|a, b| b.cmp(a));
                if sorted != STANDARD_ARRAY {
                    issues.push(err(
                        "char.scores.standard_array",
                        "Scores must be 15, 14, 13, 12, 10, 8 in any order",
                    ));
                }
            }
            ScoreMethod::PointBuy => match base
                .iter()
                .map(|s| point_cost(*s))
                .collect::<Option<Vec<_>>>()
            {
                None => issues.push(err(
                    "char.scores.point_buy_range",
                    "Point-buy scores must be between 8 and 15",
                )),
                Some(costs) => {
                    let total: u32 = costs.iter().sum();
                    if total > POINT_BUY_BUDGET {
                        issues.push(err(
                            "char.scores.point_buy_budget",
                            format!("Spent {total} of {POINT_BUY_BUDGET} points"),
                        ));
                    }
                }
            },
            ScoreMethod::Rolled => {
                if base.iter().any(|s| !(3..=18).contains(s)) {
                    issues.push(err(
                        "char.scores.rolled_range",
                        "Rolled scores must be 3–18",
                    ));
                }
            }
            ScoreMethod::ManualDmApproved => {
                issues.push(warn(
                    "char.scores.manual",
                    "Manually entered scores (DM override)",
                ));
            }
        }

        // Background adjustment.
        let bg = self.background_def();
        if let BackgroundBonus::PlusTwoPlusOne { plus_two, plus_one } = self.background_bonus {
            if plus_two == plus_one {
                issues.push(err(
                    "char.background.same_ability",
                    "+2 and +1 must go to different abilities",
                ));
            }
            if !bg.abilities.contains(&plus_two) || !bg.abilities.contains(&plus_one) {
                issues.push(err(
                    "char.background.ability",
                    format!("{} only allows {:?}", bg.name, bg.abilities),
                ));
            }
        }
        let raw_after: Vec<u8> = match self.background_bonus {
            BackgroundBonus::PlusTwoPlusOne { plus_two, plus_one } => Ability::ALL
                .iter()
                .map(|a| {
                    base[a.index()]
                        + if *a == plus_two { 2 } else { 0 }
                        + if *a == plus_one { 1 } else { 0 }
                })
                .collect(),
            BackgroundBonus::ThreePlusOnes => Ability::ALL
                .iter()
                .map(|a| base[a.index()] + u8::from(bg.abilities.contains(a)))
                .collect(),
        };
        if raw_after.iter().any(|s| *s > 20) {
            issues.push(err(
                "char.scores.max20",
                "Background increases can't raise a score above 20",
            ));
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
            issues.push(err(
                "char.skills.count",
                format!("{} chooses {} skills", cd.name, cd.skill_choices),
            ));
        }
        if !cd.skill_list.is_empty() {
            for s in &self.class_skills {
                if !cd.skill_list.contains(s) {
                    issues.push(err(
                        "char.skills.not_in_list",
                        format!("{s:?} isn't a {} skill option", cd.name),
                    ));
                }
            }
        }
        for s in &self.class_skills {
            if bg.skills.contains(s) {
                issues.push(warn(
                    "char.skills.overlap",
                    format!("{s:?} is already granted by the background; choose another"),
                ));
            }
        }
        match (self.species, self.species_skill) {
            (SpeciesId::Human, None) => issues.push(err(
                "char.species.skillful",
                "Human Skillful: choose one skill",
            )),
            (SpeciesId::Human, Some(_)) => {}
            (_, Some(_)) => issues.push(err(
                "char.species.extra_skill",
                "Only Humans gain the Skillful skill",
            )),
            _ => {}
        }

        // Equipment.
        if let Some(id) = &self.body_armor {
            match equipment::armor(id) {
                None => issues.push(err("char.armor.unknown", format!("Unknown armor {id}"))),
                Some(a) if a.category == ArmorCategory::Shield => {
                    issues.push(err("char.armor.shield_as_body", "Use the shield flag"))
                }
                Some(a) => {
                    if !cd.trained_in_armor(a.category) {
                        issues.push(warn("char.armor.untrained", format!("No training with {:?} armor: Disadvantage on Str/Dex D20 Tests and no spellcasting", a.category)));
                    }
                    if let Some(req) = a.strength_req
                        && self.scores().get(Ability::Strength) < req
                    {
                        issues.push(warn(
                            "char.armor.strength",
                            format!("{} needs Str {req}: Speed reduced by 10 ft", a.name),
                        ));
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
        let mastery_count = crate::classes::weapon_mastery_count(self.class, self.level);
        if self.weapon_masteries.len() > mastery_count as usize {
            issues.push(err(
                "char.mastery.count",
                format!(
                    "{} masters {} weapon kinds at level {}",
                    cd.name, mastery_count, self.level
                ),
            ));
        }
        for m in &self.weapon_masteries {
            match equipment::weapon(m) {
                None => issues.push(err("char.mastery.unknown", format!("Unknown weapon {m}"))),
                Some(w) => {
                    if !cd.proficient_with(w) {
                        issues.push(err(
                            "char.mastery.not_proficient",
                            format!("Not proficient with {}", w.name),
                        ));
                    }
                    if self.class == ClassId::Barbarian && w.kind != WeaponKind::Melee {
                        issues.push(err(
                            "char.mastery.barbarian_melee",
                            "Barbarian masteries must be melee weapons",
                        ));
                    }
                }
            }
        }
        issues
    }

    pub fn is_valid(&self) -> bool {
        self.validate()
            .iter()
            .all(|i| i.severity != Severity::Error)
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
            background_bonus: BackgroundBonus::PlusTwoPlusOne {
                plus_two: Ability::Strength,
                plus_one: Ability::Constitution,
            },
            class_skills: vec![Skill::Perception, Skill::Survival],
            species_skill: None,
            body_armor: Some("chain_mail".into()),
            shield: true,
            weapons: vec!["longsword".into(), "javelin".into(), "greatsword".into()],
            weapon_masteries: vec!["longsword".into(), "greatsword".into(), "javelin".into()],
            experience_points: 0,
            biography: String::new(),
            subclass: None,
            asi: vec![],
            fighting_style: None,
            expertise: vec![],
            cantrips: vec![],
            prepared_spells: vec![],
            inventory: vec![],
            currency: Default::default(),
        }
    }

    /// Level-dependent choices: subclass, ASI, fighting style, expertise, spells.
    fn validate_progression(&self, issues: &mut Vec<Issue>) {
        use crate::classes;
        let srd_sub = classes::srd_subclass(self.class);
        match (&self.subclass, self.level >= 3) {
            (Some(s), true) if s != srd_sub => issues.push(err(
                "char.subclass",
                format!("Only the SRD subclass {srd_sub} is available"),
            )),
            (Some(_), false) => issues.push(err(
                "char.subclass.level",
                "Subclasses are chosen at level 3",
            )),
            (None, true) => issues.push(warn(
                "char.subclass.missing",
                format!("Choose your subclass ({srd_sub})"),
            )),
            _ => {}
        }
        let asi_levels = (1..=self.level)
            .filter(|l| classes::is_asi_level(self.class, *l))
            .count() as u32;
        let asi_points: u32 = self.asi.iter().map(|(_, n)| *n as u32).sum();
        if self.asi.iter().any(|(_, n)| !(1..=2).contains(n)) || asi_points > asi_levels * 2 {
            issues.push(err(
                "char.asi",
                format!(
                    "{asi_levels} Ability Score Improvement(s) allow {} points",
                    asi_levels * 2
                ),
            ));
        }
        let style_level = match self.class {
            ClassId::Fighter => Some(1),
            ClassId::Paladin | ClassId::Ranger => Some(2),
            _ => None,
        };
        match (self.fighting_style, style_level) {
            (Some(_), None) => issues.push(err(
                "char.style.class",
                "Only Fighters, Paladins and Rangers have the Fighting Style feature",
            )),
            (Some(_), Some(l)) if self.level < l => issues.push(err(
                "char.style.level",
                format!("Fighting Style comes at level {l}"),
            )),
            (None, Some(l)) if self.level >= l => {
                issues.push(warn("char.style.missing", "Choose a Fighting Style feat"))
            }
            _ => {}
        }
        let expertise_allowed = match self.class {
            ClassId::Rogue => {
                if self.level >= 6 {
                    4
                } else {
                    2
                }
            }
            ClassId::Bard if self.level >= 9 => 4,
            ClassId::Bard if self.level >= 2 => 2,
            ClassId::Ranger if self.level >= 9 => 2,
            _ => 0,
        };
        if self.expertise.len() > expertise_allowed
            || self.expertise.iter().any(|s| !self.skill_proficient(*s))
        {
            issues.push(err(
                "char.expertise",
                format!(
                    "Up to {expertise_allowed} Expertise skills, each one you're proficient in"
                ),
            ));
        }
        // Spells.
        let Some((_, _)) = classes::spellcasting(self.class) else {
            if !self.cantrips.is_empty() || !self.prepared_spells.is_empty() {
                issues.push(err(
                    "char.spells.noncaster",
                    format!("{} doesn't cast spells", self.class_def().name),
                ));
            }
            return;
        };
        let class_name = self.class_def().name;
        let max_cantrips = classes::cantrips_known(self.class, self.level) as usize;
        let max_prepared = classes::prepared_spells(self.class, self.level) as usize;
        let max_level = classes::max_spell_level(self.class, self.level);
        if self.cantrips.len() > max_cantrips {
            issues.push(err(
                "char.spells.cantrips",
                format!(
                    "{class_name} level {} knows {max_cantrips} cantrips",
                    self.level
                ),
            ));
        }
        if self.prepared_spells.len() > max_prepared {
            issues.push(err(
                "char.spells.prepared",
                format!(
                    "{class_name} level {} prepares {max_prepared} spells",
                    self.level
                ),
            ));
        }
        for id in self.cantrips.iter().chain(&self.prepared_spells) {
            match crate::spells::spell(id) {
                None => issues.push(err("char.spells.unknown", format!("Unknown spell {id}"))),
                Some(sp) => {
                    if !sp.classes.iter().any(|c| c == class_name) {
                        issues.push(err(
                            "char.spells.list",
                            format!("{} isn't on the {class_name} spell list", sp.name),
                        ));
                    }
                    let is_cantrip_slot = self.cantrips.contains(id);
                    if is_cantrip_slot != (sp.level == 0) {
                        issues.push(err(
                            "char.spells.kind",
                            format!("{} is in the wrong list (cantrip vs prepared)", sp.name),
                        ));
                    }
                    if sp.level > max_level {
                        issues.push(err(
                            "char.spells.level",
                            format!(
                                "{} is level {}; you can cast up to level {max_level}",
                                sp.name, sp.level
                            ),
                        ));
                    }
                }
            }
        }
        if self.cantrips.len() < max_cantrips || self.prepared_spells.len() < max_prepared {
            issues.push(warn(
                "char.spells.unfilled",
                format!("Choose {max_cantrips} cantrips and {max_prepared} prepared spells"),
            ));
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
        assert_eq!(
            (s.get(Ability::Strength), s.get(Ability::Constitution)),
            (17, 15)
        );
        // Fighter: 10 + Con mod (+2) + Dwarven Toughness (+1)
        assert_eq!(c.max_hp(), 13);
        assert_eq!(c.armor_class(), 18); // Chain Mail 16 + Shield 2
        assert_eq!(c.save_bonus(Ability::Strength), 3 + 2);
        assert_eq!(c.save_bonus(Ability::Wisdom), 1);
        assert_eq!(
            c.skill_bonus(Skill::Athletics),
            3 + 2,
            "Soldier grants Athletics"
        );
        assert_eq!(c.darkvision_ft(), 120);
        let ls = c
            .attacks()
            .into_iter()
            .find(|a| a.name == "Longsword")
            .unwrap();
        assert_eq!(ls.to_hit, 5);
        assert_eq!(ls.damage[0].dice.to_string(), "1d8+3");
        assert_eq!(ls.mastery, Some(Mastery::Sap));
        let unarmed = c
            .attacks()
            .into_iter()
            .find(|a| a.name == "Unarmed Strike")
            .unwrap();
        assert_eq!(unarmed.damage[0].dice.to_string(), "4");
        assert!(
            c.attacks()
                .iter()
                .any(|a| a.name == "Javelin (thrown)" && a.range_ft == Some((30, 120)))
        );
    }

    #[test]
    fn rejects_invalid_scores_and_background_choices() {
        let mut c = CharacterSheet::example_fighter("X");
        c.base_scores = AbilityScores::new(15, 15, 13, 12, 10, 8);
        assert!(!c.is_valid());

        let mut c = CharacterSheet::example_fighter("X");
        c.background_bonus = BackgroundBonus::PlusTwoPlusOne {
            plus_two: Ability::Charisma,
            plus_one: Ability::Strength,
        };
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
        for (class, expected) in [
            (ClassId::Barbarian, 12),
            (ClassId::Ranger, 10),
            (ClassId::Rogue, 8),
            (ClassId::Wizard, 6),
        ] {
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

    fn with(class: ClassId, level: u8, f: impl FnOnce(&mut CharacterSheet)) -> CharacterSheet {
        let mut c = CharacterSheet::example_fighter("T");
        c.class = class;
        c.level = level;
        c.weapon_masteries.clear();
        c.class_skills = content::class(class)
            .skill_list
            .iter()
            .copied()
            .filter(|s| !c.background_def().skills.contains(s))
            .take(content::class(class).skill_choices as usize)
            .collect();
        if class == ClassId::Bard {
            c.class_skills = vec![Skill::Acrobatics, Skill::Arcana, Skill::Deception];
        }
        f(&mut c);
        c
    }

    #[test]
    fn unarmored_defense_and_fast_movement() {
        // Dwarf, Str 17 Dex 13 Con 15: Barbarian unarmored AC = 10 + 1 + 2 (+2 shield)
        let b = with(ClassId::Barbarian, 5, |c| {
            c.body_armor = None;
            c.shield = false;
        });
        assert_eq!(b.armor_class(), 13);
        assert_eq!(b.speed_ft(), 40, "Fast Movement at level 5");
        let b1 = with(ClassId::Barbarian, 4, |c| c.body_armor = None);
        assert_eq!(b1.speed_ft(), 30);
        let monk = with(ClassId::Monk, 2, |c| {
            c.body_armor = None;
            c.shield = false;
            c.base_scores = AbilityScores::new(8, 15, 13, 10, 14, 12);
        });
        // Dex 15 → +2, Wis 14 → +2 → AC 14; Unarmored Movement +10.
        assert_eq!(monk.armor_class(), 14);
        assert_eq!(monk.speed_ft(), 40);
        let unarmed = monk
            .attacks()
            .into_iter()
            .find(|a| a.name == "Unarmed Strike")
            .unwrap();
        assert_eq!(
            unarmed.damage[0].dice.to_string(),
            "1d6+2",
            "Martial Arts die + Dex"
        );
    }

    #[test]
    fn champion_crits_and_extra_attack() {
        let f = with(ClassId::Fighter, 3, |c| {
            c.subclass = Some("Champion".into())
        });
        assert_eq!(f.crit_threshold(), 19);
        assert_eq!(with(ClassId::Fighter, 2, |_| {}).crit_threshold(), 20);
        assert_eq!(
            with(ClassId::Fighter, 15, |c| c.subclass =
                Some("Champion".into()))
            .crit_threshold(),
            18
        );
        assert_eq!(with(ClassId::Fighter, 11, |_| {}).attacks_per_action(), 3);
        let archer = with(ClassId::Fighter, 1, |c| {
            c.fighting_style = Some(FightingStyle::Archery);
            c.weapons = vec!["longbow".into()];
        });
        let bow = archer
            .attacks()
            .into_iter()
            .find(|a| a.name == "Longbow")
            .unwrap();
        assert_eq!(bow.to_hit, 1 + 2 + 2, "Dex +1, PB +2, Archery +2");
        let defense = with(ClassId::Fighter, 1, |c| {
            c.fighting_style = Some(FightingStyle::Defense)
        });
        assert_eq!(defense.armor_class(), 19);
    }

    #[test]
    fn ability_score_improvements_are_limited() {
        let mut f = with(ClassId::Fighter, 4, |c| {
            c.asi = vec![(Ability::Strength, 2)]
        });
        assert_eq!(f.scores().get(Ability::Strength), 19);
        f.asi = vec![(Ability::Strength, 2), (Ability::Dexterity, 1)];
        assert!(
            f.validate().iter().any(|i| i.key == "char.asi"),
            "level 4 gives only 2 points"
        );
        let f6 = with(ClassId::Fighter, 6, |c| {
            c.asi = vec![(Ability::Strength, 2), (Ability::Constitution, 2)]
        });
        assert!(
            !f6.validate().iter().any(|i| i.key == "char.asi"),
            "Fighter has ASIs at 4 and 6"
        );
        let capped = with(ClassId::Fighter, 8, |c| {
            c.asi = vec![(Ability::Strength, 2), (Ability::Strength, 2)]
        });
        assert_eq!(
            capped.scores().get(Ability::Strength),
            20,
            "can't exceed 20"
        );
    }

    #[test]
    fn wizard_spells_validate_against_list_and_counts() {
        let mut w = with(ClassId::Wizard, 1, |c| {
            c.weapons = vec!["dagger".into()];
            c.body_armor = None;
            c.shield = false;
            c.cantrips = vec!["fire_bolt".into(), "light".into(), "ray_of_frost".into()];
            c.prepared_spells = vec![
                "magic_missile".into(),
                "shield".into(),
                "mage_armor".into(),
                "burning_hands".into(),
            ];
        });
        assert!(w.is_valid(), "{:?}", w.validate());
        // Int 8 - 1 + PB 2 = DC 9
        assert_eq!(w.spell_save_dc(), Some(8 - 1 + 2));
        w.prepared_spells.push("sleep".into());
        assert!(!w.is_valid(), "5 prepared > 4");
        w.prepared_spells = vec!["fireball".into()];
        assert!(!w.is_valid(), "level 3 spell at character level 1");
        w.prepared_spells = vec!["cure_wounds".into()];
        assert!(!w.is_valid(), "not on the Wizard list");
        let fighter = with(ClassId::Fighter, 1, |c| {
            c.cantrips = vec!["fire_bolt".into()]
        });
        assert!(!fighter.is_valid());
    }

    #[test]
    fn always_prepared_spells() {
        let p = with(ClassId::Paladin, 2, |_| {});
        assert!(p.knows_spell("divine_smite"));
        let cl = with(ClassId::Cleric, 3, |c| {
            c.subclass = Some("Life Domain".into())
        });
        assert!(cl.knows_spell("bless") && cl.knows_spell("cure_wounds"));
    }

    #[test]
    fn draconic_resilience() {
        let s = with(ClassId::Sorcerer, 3, |c| {
            c.subclass = Some("Draconic Sorcery".into());
            c.body_armor = None;
            c.shield = false;
            c.weapons = vec![];
        });
        // Sorcerer d6: 6+2 +2×(4+2) + Dwarven Toughness 3 + Draconic 3
        assert_eq!(s.max_hp(), 8 + 12 + 3 + 3);
        assert_eq!(s.armor_class(), 10 + 1 + 0);
    }
}
