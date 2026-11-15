//! SRD 5.2.1 character-origin and class data needed for level-1 character creation.
//!
//! Only data verified against SRD 5.2.1 is included. Class features beyond the core traits table
//! (Rage, Spellcasting, Sneak Attack, ...) are tracked as pending in `docs/RULES_COVERAGE.md`.

use crate::ability::{Ability, Skill};
use crate::equipment::{ArmorCategory, Weapon, WeaponCategory, WeaponProperty};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ClassId {
    Barbarian,
    Bard,
    Cleric,
    Druid,
    Fighter,
    Monk,
    Paladin,
    Ranger,
    Rogue,
    Sorcerer,
    Warlock,
    Wizard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponTraining {
    Simple,
    SimpleAndMartial,
    /// Monk: Simple weapons and Martial weapons that have the Light property.
    SimpleAndMartialLight,
    /// Rogue: Simple weapons and Martial weapons that have the Finesse or Light property.
    SimpleAndMartialFinesseOrLight,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: ClassId,
    pub name: &'static str,
    pub hit_die: u32,
    pub saving_throws: [Ability; 2],
    pub skill_choices: u8,
    /// Empty = choose from any skill (Bard).
    pub skill_list: &'static [Skill],
    pub weapons: WeaponTraining,
    pub armor: &'static [ArmorCategory],
    /// Level 1 Weapon Mastery feature: number of weapon kinds.
    pub weapon_mastery_count: u8,
}

impl ClassDef {
    pub fn proficient_with(&self, w: &Weapon) -> bool {
        match self.weapons {
            WeaponTraining::Simple => w.category == WeaponCategory::Simple,
            WeaponTraining::SimpleAndMartial => true,
            WeaponTraining::SimpleAndMartialLight => {
                w.category == WeaponCategory::Simple || w.has(WeaponProperty::Light)
            }
            WeaponTraining::SimpleAndMartialFinesseOrLight => {
                w.category == WeaponCategory::Simple
                    || w.has(WeaponProperty::Light)
                    || w.has(WeaponProperty::Finesse)
            }
        }
    }
    pub fn trained_in_armor(&self, c: ArmorCategory) -> bool {
        self.armor.contains(&c)
    }
}

use Ability::*;
use ArmorCategory::{Heavy, Light, Medium, Shield};
use Skill::*;

pub static CLASSES: &[ClassDef] = &[
    ClassDef {
        id: ClassId::Barbarian,
        name: "Barbarian",
        hit_die: 12,
        saving_throws: [Strength, Constitution],
        skill_choices: 2,
        skill_list: &[
            AnimalHandling,
            Athletics,
            Intimidation,
            Nature,
            Perception,
            Survival,
        ],
        weapons: WeaponTraining::SimpleAndMartial,
        armor: &[Light, Medium, Shield],
        weapon_mastery_count: 2,
    },
    ClassDef {
        id: ClassId::Bard,
        name: "Bard",
        hit_die: 8,
        saving_throws: [Dexterity, Charisma],
        skill_choices: 3,
        skill_list: &[],
        weapons: WeaponTraining::Simple,
        armor: &[Light],
        weapon_mastery_count: 0,
    },
    ClassDef {
        id: ClassId::Cleric,
        name: "Cleric",
        hit_die: 8,
        saving_throws: [Wisdom, Charisma],
        skill_choices: 2,
        skill_list: &[History, Insight, Medicine, Persuasion, Religion],
        weapons: WeaponTraining::Simple,
        armor: &[Light, Medium, Shield],
        weapon_mastery_count: 0,
    },
    ClassDef {
        id: ClassId::Druid,
        name: "Druid",
        hit_die: 8,
        saving_throws: [Intelligence, Wisdom],
        skill_choices: 2,
        skill_list: &[
            AnimalHandling,
            Arcana,
            Insight,
            Medicine,
            Nature,
            Perception,
            Religion,
            Survival,
        ],
        weapons: WeaponTraining::Simple,
        armor: &[Light, Shield],
        weapon_mastery_count: 0,
    },
    ClassDef {
        id: ClassId::Fighter,
        name: "Fighter",
        hit_die: 10,
        saving_throws: [Strength, Constitution],
        skill_choices: 2,
        skill_list: &[
            Acrobatics,
            AnimalHandling,
            Athletics,
            History,
            Insight,
            Intimidation,
            Persuasion,
            Perception,
            Survival,
        ],
        weapons: WeaponTraining::SimpleAndMartial,
        armor: &[Light, Medium, Heavy, Shield],
        weapon_mastery_count: 3,
    },
    ClassDef {
        id: ClassId::Monk,
        name: "Monk",
        hit_die: 8,
        saving_throws: [Strength, Dexterity],
        skill_choices: 2,
        skill_list: &[Acrobatics, Athletics, History, Insight, Religion, Stealth],
        weapons: WeaponTraining::SimpleAndMartialLight,
        armor: &[],
        weapon_mastery_count: 0,
    },
    ClassDef {
        id: ClassId::Paladin,
        name: "Paladin",
        hit_die: 10,
        saving_throws: [Wisdom, Charisma],
        skill_choices: 2,
        skill_list: &[
            Athletics,
            Insight,
            Intimidation,
            Medicine,
            Persuasion,
            Religion,
        ],
        weapons: WeaponTraining::SimpleAndMartial,
        armor: &[Light, Medium, Heavy, Shield],
        weapon_mastery_count: 2,
    },
    ClassDef {
        id: ClassId::Ranger,
        name: "Ranger",
        hit_die: 10,
        saving_throws: [Strength, Dexterity],
        skill_choices: 3,
        skill_list: &[
            AnimalHandling,
            Athletics,
            Insight,
            Investigation,
            Nature,
            Perception,
            Stealth,
            Survival,
        ],
        weapons: WeaponTraining::SimpleAndMartial,
        armor: &[Light, Medium, Shield],
        weapon_mastery_count: 2,
    },
    ClassDef {
        id: ClassId::Rogue,
        name: "Rogue",
        hit_die: 8,
        saving_throws: [Dexterity, Intelligence],
        skill_choices: 4,
        skill_list: &[
            Acrobatics,
            Athletics,
            Deception,
            Insight,
            Intimidation,
            Investigation,
            Perception,
            Persuasion,
            SleightOfHand,
            Stealth,
        ],
        weapons: WeaponTraining::SimpleAndMartialFinesseOrLight,
        armor: &[Light],
        weapon_mastery_count: 2,
    },
    ClassDef {
        id: ClassId::Sorcerer,
        name: "Sorcerer",
        hit_die: 6,
        saving_throws: [Constitution, Charisma],
        skill_choices: 2,
        skill_list: &[
            Arcana,
            Deception,
            Insight,
            Intimidation,
            Persuasion,
            Religion,
        ],
        weapons: WeaponTraining::Simple,
        armor: &[],
        weapon_mastery_count: 0,
    },
    ClassDef {
        id: ClassId::Warlock,
        name: "Warlock",
        hit_die: 8,
        saving_throws: [Wisdom, Charisma],
        skill_choices: 2,
        skill_list: &[
            Arcana,
            Deception,
            History,
            Intimidation,
            Investigation,
            Nature,
            Religion,
        ],
        weapons: WeaponTraining::Simple,
        armor: &[Light],
        weapon_mastery_count: 0,
    },
    ClassDef {
        id: ClassId::Wizard,
        name: "Wizard",
        hit_die: 6,
        saving_throws: [Intelligence, Wisdom],
        skill_choices: 2,
        skill_list: &[
            Arcana,
            History,
            Insight,
            Investigation,
            Medicine,
            Nature,
            Religion,
        ],
        weapons: WeaponTraining::Simple,
        armor: &[],
        weapon_mastery_count: 0,
    },
];

pub fn class(id: ClassId) -> &'static ClassDef {
    CLASSES
        .iter()
        .find(|c| c.id == id)
        .expect("all classes defined")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SpeciesId {
    Dragonborn,
    Dwarf,
    Elf,
    Gnome,
    Goliath,
    Halfling,
    Human,
    Orc,
    Tiefling,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Size {
    Tiny,
    Small,
    Medium,
    Large,
    Huge,
    Gargantuan,
}

#[derive(Debug, Clone)]
pub struct SpeciesDef {
    pub id: SpeciesId,
    pub name: &'static str,
    /// Selectable sizes (Human and Tiefling choose Small or Medium).
    pub sizes: &'static [Size],
    pub speed_ft: i32,
    pub darkvision_ft: u32,
    /// Dwarven Toughness: +1 HP max per level.
    pub hp_per_level_bonus: i32,
}

pub static SPECIES: &[SpeciesDef] = &[
    SpeciesDef {
        id: SpeciesId::Dragonborn,
        name: "Dragonborn",
        sizes: &[Size::Medium],
        speed_ft: 30,
        darkvision_ft: 60,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Dwarf,
        name: "Dwarf",
        sizes: &[Size::Medium],
        speed_ft: 30,
        darkvision_ft: 120,
        hp_per_level_bonus: 1,
    },
    SpeciesDef {
        id: SpeciesId::Elf,
        name: "Elf",
        sizes: &[Size::Medium],
        speed_ft: 30,
        darkvision_ft: 60,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Gnome,
        name: "Gnome",
        sizes: &[Size::Small],
        speed_ft: 30,
        darkvision_ft: 60,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Goliath,
        name: "Goliath",
        sizes: &[Size::Medium],
        speed_ft: 35,
        darkvision_ft: 0,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Halfling,
        name: "Halfling",
        sizes: &[Size::Small],
        speed_ft: 30,
        darkvision_ft: 0,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Human,
        name: "Human",
        sizes: &[Size::Medium, Size::Small],
        speed_ft: 30,
        darkvision_ft: 0,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Orc,
        name: "Orc",
        sizes: &[Size::Medium],
        speed_ft: 30,
        darkvision_ft: 120,
        hp_per_level_bonus: 0,
    },
    SpeciesDef {
        id: SpeciesId::Tiefling,
        name: "Tiefling",
        sizes: &[Size::Medium, Size::Small],
        speed_ft: 30,
        darkvision_ft: 60,
        hp_per_level_bonus: 0,
    },
];

pub fn species(id: SpeciesId) -> &'static SpeciesDef {
    SPECIES
        .iter()
        .find(|s| s.id == id)
        .expect("all species defined")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BackgroundId {
    Acolyte,
    Criminal,
    Sage,
    Soldier,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OriginFeat {
    Alert,
    MagicInitiateCleric,
    MagicInitiateWizard,
    SavageAttacker,
}

#[derive(Debug, Clone)]
pub struct BackgroundDef {
    pub id: BackgroundId,
    pub name: &'static str,
    pub abilities: [Ability; 3],
    pub feat: OriginFeat,
    pub skills: [Skill; 2],
}

pub static BACKGROUNDS: &[BackgroundDef] = &[
    BackgroundDef {
        id: BackgroundId::Acolyte,
        name: "Acolyte",
        abilities: [Intelligence, Wisdom, Charisma],
        feat: OriginFeat::MagicInitiateCleric,
        skills: [Insight, Religion],
    },
    BackgroundDef {
        id: BackgroundId::Criminal,
        name: "Criminal",
        abilities: [Dexterity, Constitution, Intelligence],
        feat: OriginFeat::Alert,
        skills: [SleightOfHand, Stealth],
    },
    BackgroundDef {
        id: BackgroundId::Sage,
        name: "Sage",
        abilities: [Constitution, Intelligence, Wisdom],
        feat: OriginFeat::MagicInitiateWizard,
        skills: [Arcana, History],
    },
    BackgroundDef {
        id: BackgroundId::Soldier,
        name: "Soldier",
        abilities: [Strength, Dexterity, Constitution],
        feat: OriginFeat::SavageAttacker,
        skills: [Athletics, Intimidation],
    },
];

pub fn background(id: BackgroundId) -> &'static BackgroundDef {
    BACKGROUNDS
        .iter()
        .find(|b| b.id == id)
        .expect("all backgrounds defined")
}
