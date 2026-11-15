//! Monster stat blocks (2024 format). The bestiary is the full validated SRD 5.2.1 monster list
//! (`content/srd_monsters.json`, extracted by `tools/extract_monsters.py`), plus DM custom monsters.

use crate::ability::{Ability, AbilityScores};
use crate::attack::{AttackProfile, DamagePart};
use crate::conditions::Condition;
use crate::content::Size;
use crate::damage::{DamageType, Defenses};
use crate::dice::DiceExpr;
use crate::equipment::WeaponKind;
use crate::spells::{Area, AreaShape};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

/// A save-based action such as a breath weapon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveAction {
    pub name: String,
    pub ability: Ability,
    pub dc: i32,
    pub damage: Vec<DamagePart>,
    pub half_on_success: bool,
    pub area: Option<Area>,
    pub conditions: Vec<Condition>,
    /// "Recharge 5–6" → Some(5).
    pub recharge: Option<u8>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TextEntry {
    pub name: String,
    pub text: String,
}

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
    #[serde(default)]
    pub saves: [i32; 6],
    #[serde(default)]
    pub speeds: String,
    #[serde(default)]
    pub blindsight_ft: u32,
    #[serde(default)]
    pub truesight_ft: u32,
    #[serde(default)]
    pub senses: String,
    #[serde(default)]
    pub languages: String,
    #[serde(default)]
    pub skills: String,
    #[serde(default)]
    pub tags: String,
    #[serde(default)]
    pub traits: Vec<TextEntry>,
    #[serde(default)]
    pub actions: Vec<TextEntry>,
    #[serde(default)]
    pub bonus_actions: Vec<TextEntry>,
    #[serde(default)]
    pub reactions: Vec<TextEntry>,
    #[serde(default)]
    pub legendary_actions: Vec<TextEntry>,
    #[serde(default)]
    pub save_actions: Vec<SaveAction>,
    #[serde(default = "one")]
    pub attacks_per_action: u8,
    #[serde(default)]
    pub exhaustion_immune: bool,
}

fn one() -> u8 {
    1
}

impl MonsterTemplate {
    pub fn initiative_score(&self) -> i32 {
        10 + self.initiative_bonus
    }

    /// Challenge Rating as a number for sorting/filtering (1/8 → 0.125).
    pub fn cr_value(&self) -> f32 {
        match self.challenge.split_once('/') {
            Some((a, b)) => a.parse::<f32>().unwrap_or(0.0) / b.parse::<f32>().unwrap_or(1.0),
            None => self.challenge.parse().unwrap_or(0.0),
        }
    }

    pub fn save_bonus(&self, a: Ability) -> i32 {
        self.saves[a.index()]
    }

    /// A blank homebrew template for the custom-monster editor.
    pub fn blank_custom(id: &str) -> Self {
        MonsterTemplate {
            id: id.into(),
            name: "New Monster".into(),
            size: Size::Medium,
            creature_type: "Monstrosity".into(),
            alignment: "Unaligned".into(),
            armor_class: 12,
            hit_points: 11,
            hit_dice: DiceExpr::dice(2, 8).with_modifier(2),
            speed_ft: 30,
            initiative_bonus: 1,
            scores: AbilityScores::new(14, 12, 14, 6, 10, 7),
            defenses: Defenses::default(),
            condition_immunities: BTreeSet::new(),
            darkvision_ft: 60,
            passive_perception: 10,
            challenge: "1/2".into(),
            xp: 100,
            proficiency_bonus: 2,
            attacks: vec![AttackProfile {
                name: "Claw".into(),
                kind: WeaponKind::Melee,
                to_hit: 4,
                damage: vec![DamagePart { dice: DiceExpr::dice(1, 6).with_modifier(2), damage_type: DamageType::Slashing }],
                extra_damage_with_advantage: vec![],
                reach_ft: 5,
                range_ft: None,
                mastery: None,
                ability_mod: 2,
                ..Default::default()
            }],
            notes: vec![],
            source: "homebrew".into(),
            saves: [2, 1, 2, -2, 0, -2],
            speeds: "30 ft.".into(),
            blindsight_ft: 0,
            truesight_ft: 0,
            senses: String::new(),
            languages: String::new(),
            skills: String::new(),
            tags: String::new(),
            traits: vec![],
            actions: vec![],
            bonus_actions: vec![],
            reactions: vec![],
            legendary_actions: vec![],
            save_actions: vec![],
            attacks_per_action: 1,
            exhaustion_immune: false,
        }
    }

    /// Basic checks for DM-made monsters (keeps garbage out of saves and the network).
    pub fn validate_custom(&self) -> Result<(), String> {
        if self.name.trim().is_empty() || self.name.len() > 60 {
            return Err("name must be 1–60 characters".into());
        }
        if !(1..=40).contains(&self.armor_class) || !(1..=2000).contains(&self.hit_points) {
            return Err("AC must be 1–40 and HP 1–2000".into());
        }
        if self.scores.0.iter().any(|s| !(1..=30).contains(s)) {
            return Err("ability scores must be 1–30".into());
        }
        if self.attacks.len() > 8 || self.attacks.iter().any(|a| a.damage.is_empty() || a.to_hit.abs() > 30) {
            return Err("up to 8 attacks, each with damage and a to-hit between -30 and +30".into());
        }
        Ok(())
    }
}

// ------------------------------------------------------------------ JSON import

#[derive(Deserialize)]
struct RawAbility {
    score: u8,
    save: i32,
}

#[derive(Deserialize)]
struct RawDamage {
    dice: String,
    #[serde(rename = "type")]
    damage_type: DamageType,
}

#[derive(Deserialize)]
struct RawAttack {
    name: String,
    kind: String,
    to_hit: i32,
    reach_ft: u32,
    range_ft: Option<(u32, u32)>,
    damage: Vec<RawDamage>,
    extra_with_advantage: Vec<RawDamage>,
    thrown: bool,
}

#[derive(Deserialize)]
struct RawSaveAction {
    name: String,
    ability: Ability,
    dc: i32,
    damage: Vec<RawDamage>,
    half_on_success: bool,
    area: Option<Area>,
    conditions: Vec<Condition>,
}

#[derive(Deserialize)]
struct RawMonster {
    id: String,
    name: String,
    size: String,
    #[serde(rename = "type")]
    creature_type: String,
    tags: Option<String>,
    alignment: String,
    ac: i32,
    hp: i32,
    hit_dice: String,
    speed: Option<String>,
    speed_ft: i32,
    initiative: i32,
    abilities: std::collections::HashMap<String, RawAbility>,
    cr: String,
    xp: u32,
    pb: i32,
    skills: Option<String>,
    senses: Option<String>,
    languages: Option<String>,
    damage_immunities: Vec<DamageType>,
    condition_immunities: Vec<Condition>,
    damage_resistances: Vec<DamageType>,
    damage_vulnerabilities: Vec<DamageType>,
    darkvision_ft: u32,
    blindsight_ft: u32,
    truesight_ft: u32,
    passive_perception: i32,
    traits: Vec<TextEntry>,
    actions: Vec<TextEntry>,
    bonus_actions: Vec<TextEntry>,
    reactions: Vec<TextEntry>,
    legendary_actions: Vec<TextEntry>,
    attacks: Vec<RawAttack>,
    save_actions: Vec<RawSaveAction>,
    attacks_per_action: u8,
    #[serde(default)]
    exhaustion_immune: bool,
}

fn parse_dice(s: &str) -> DiceExpr {
    DiceExpr::parse(s).unwrap_or_default()
}

fn parts(v: &[RawDamage]) -> Vec<DamagePart> {
    v.iter().map(|d| DamagePart { dice: parse_dice(&d.dice), damage_type: d.damage_type }).collect()
}

fn convert(r: RawMonster) -> MonsterTemplate {
    let ab = |k: &str| r.abilities.get(k).map(|a| (a.score, a.save)).unwrap_or((10, 0));
    let keys = ["str", "dex", "con", "int", "wis", "cha"];
    let scores = AbilityScores(keys.map(|k| ab(k).0));
    let saves = keys.map(|k| ab(k).1);
    let size = match r.size.split_whitespace().next().unwrap_or("Medium") {
        "Tiny" => Size::Tiny,
        "Small" => Size::Small,
        "Large" => Size::Large,
        "Huge" => Size::Huge,
        "Gargantuan" => Size::Gargantuan,
        _ => Size::Medium,
    };
    let mut attacks = Vec::new();
    for a in &r.attacks {
        let ranged = a.kind == "ranged";
        let profile = AttackProfile {
            name: a.name.clone(),
            kind: if ranged { WeaponKind::Ranged } else { WeaponKind::Melee },
            to_hit: a.to_hit,
            damage: parts(&a.damage),
            extra_damage_with_advantage: parts(&a.extra_with_advantage),
            reach_ft: a.reach_ft,
            range_ft: if ranged { a.range_ft } else { None },
            mastery: None,
            ability_mod: a.to_hit - r.pb,
            ..Default::default()
        };
        if a.thrown {
            let mut thrown = profile.clone();
            thrown.name = format!("{} (thrown)", a.name);
            thrown.kind = WeaponKind::Ranged;
            thrown.range_ft = a.range_ft;
            thrown.reach_ft = 0;
            attacks.push(profile);
            attacks.push(thrown);
        } else {
            attacks.push(profile);
        }
    }
    let save_actions = r
        .save_actions
        .iter()
        .map(|s| SaveAction {
            name: s.name.clone(),
            ability: s.ability,
            dc: s.dc,
            damage: parts(&s.damage),
            half_on_success: s.half_on_success,
            area: s.area,
            conditions: s.conditions.clone(),
            recharge: s.name.find("Recharge ").and_then(|i| s.name[i + 9..].chars().next()).and_then(|c| c.to_digit(10)).map(|d| d as u8),
            text: r.actions.iter().chain(&r.bonus_actions).find(|e| e.name == s.name).map(|e| e.text.clone()).unwrap_or_default(),
        })
        .collect();
    MonsterTemplate {
        id: r.id,
        name: r.name,
        size,
        creature_type: r.creature_type,
        alignment: r.alignment,
        armor_class: r.ac,
        hit_points: r.hp,
        hit_dice: parse_dice(&r.hit_dice),
        speed_ft: r.speed_ft,
        initiative_bonus: r.initiative,
        scores,
        defenses: Defenses {
            resistances: r.damage_resistances.into_iter().collect(),
            vulnerabilities: r.damage_vulnerabilities.into_iter().collect(),
            immunities: r.damage_immunities.into_iter().collect(),
            resist_all: false,
        },
        condition_immunities: r.condition_immunities.into_iter().collect(),
        darkvision_ft: r.darkvision_ft,
        passive_perception: r.passive_perception,
        challenge: r.cr,
        xp: r.xp,
        proficiency_bonus: r.pb,
        attacks,
        notes: vec![],
        source: "srd-5.2.1".into(),
        saves,
        speeds: r.speed.unwrap_or_default(),
        blindsight_ft: r.blindsight_ft,
        truesight_ft: r.truesight_ft,
        senses: r.senses.unwrap_or_default(),
        languages: r.languages.unwrap_or_default(),
        skills: r.skills.unwrap_or_default(),
        tags: r.tags.unwrap_or_default(),
        traits: r.traits,
        actions: r.actions,
        bonus_actions: r.bonus_actions,
        reactions: r.reactions,
        legendary_actions: r.legendary_actions,
        save_actions,
        attacks_per_action: r.attacks_per_action.max(1),
        exhaustion_immune: r.exhaustion_immune,
    }
}

static BESTIARY: OnceLock<Vec<MonsterTemplate>> = OnceLock::new();

/// The SRD 5.2.1 bestiary (328 validated stat blocks).
pub fn srd_bestiary() -> Vec<MonsterTemplate> {
    srd_bestiary_ref().to_vec()
}

pub fn srd_bestiary_ref() -> &'static [MonsterTemplate] {
    BESTIARY.get_or_init(|| {
        let raw: Vec<RawMonster> = serde_json::from_str(include_str!("../../../content/srd_monsters.json")).expect("valid monster data");
        raw.into_iter().map(convert).collect()
    })
}

pub fn srd_monster(id: &str) -> Option<&'static MonsterTemplate> {
    srd_bestiary_ref().iter().find(|m| m.id == id)
}

/// Area shapes used by monster actions share the spell area type.
pub fn is_area(a: &Option<Area>, shape: AreaShape) -> bool {
    a.is_some_and(|a| a.shape == shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_srd_bestiary_loads_and_is_consistent() {
        let b = srd_bestiary_ref();
        assert_eq!(b.len(), 328);
        for m in b {
            assert_eq!(m.hit_dice.average().floor() as i32, m.hit_points, "{}", m.name);
            assert!(m.armor_class >= 5 && m.armor_class <= 25, "{}", m.name);
        }
        let ids: std::collections::HashSet<_> = b.iter().map(|m| &m.id).collect();
        assert_eq!(ids.len(), b.len(), "unique ids");
    }

    #[test]
    fn goblin_and_skeleton_match_srd() {
        let g = srd_monster("goblin_warrior").unwrap();
        assert_eq!((g.armor_class, g.hit_points, g.initiative_bonus, g.challenge.as_str(), g.xp), (15, 10, 2, "1/4", 50));
        let scim = &g.attacks[0];
        assert_eq!((scim.name.as_str(), scim.to_hit, scim.damage[0].dice.to_string()), ("Scimitar", 4, "1d6+2".into()));
        assert_eq!(scim.extra_damage_with_advantage[0].dice.to_string(), "1d4");
        let s = srd_monster("skeleton").unwrap();
        assert!(s.defenses.vulnerabilities.contains(&DamageType::Bludgeoning));
        assert!(s.defenses.immunities.contains(&DamageType::Poison));
        assert!(s.condition_immunities.contains(&Condition::Poisoned));
        assert_eq!(s.saves[Ability::Dexterity.index()], 3);
    }

    #[test]
    fn dragon_breath_and_multiattack() {
        let d = srd_monster("adult_red_dragon").unwrap();
        assert_eq!(d.attacks_per_action, 3);
        let breath = d.save_actions.iter().find(|s| s.name.starts_with("Fire Breath")).unwrap();
        assert_eq!((breath.ability, breath.dc, breath.recharge), (Ability::Dexterity, 21, Some(5)));
        assert_eq!(breath.area, Some(Area { size_ft: 60, shape: AreaShape::Cone }));
        assert!(breath.half_on_success);
        assert_eq!(d.cr_value(), 17.0);
        assert_eq!(srd_monster("goblin_warrior").unwrap().cr_value(), 0.25);
    }

    #[test]
    fn custom_monster_validation() {
        let mut m = MonsterTemplate::blank_custom("custom_1");
        assert!(m.validate_custom().is_ok());
        m.hit_points = 0;
        assert!(m.validate_custom().is_err());
    }
}
