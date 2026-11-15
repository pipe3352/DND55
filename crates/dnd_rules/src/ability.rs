//! Abilities, modifiers, skills and proficiency bonus. (SRD 5.2.1 "Playing the Game": Ability Scores and Modifiers, Proficiency)

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ability {
    Strength,
    Dexterity,
    Constitution,
    Intelligence,
    Wisdom,
    Charisma,
}

impl Ability {
    pub const ALL: [Ability; 6] = [
        Ability::Strength,
        Ability::Dexterity,
        Ability::Constitution,
        Ability::Intelligence,
        Ability::Wisdom,
        Ability::Charisma,
    ];
    pub fn index(self) -> usize {
        self as usize
    }
    /// Short English abbreviation; UI layers should localize via the key `ability.<abbr>`.
    pub fn abbr(self) -> &'static str {
        ["str", "dex", "con", "int", "wis", "cha"][self.index()]
    }
}

/// Ability modifier: (score - 10) / 2, rounded down. ("Ability Scores and Modifiers" table.)
pub fn ability_modifier(score: u8) -> i32 {
    (score as i32 - 10).div_euclid(2)
}

/// Proficiency Bonus by character level (or Challenge Rating band for monsters):
/// +2 at 1–4, +3 at 5–8, +4 at 9–12, +5 at 13–16, +6 at 17–20.
pub fn proficiency_bonus(level: u8) -> i32 {
    let level = level.clamp(1, 20) as i32;
    2 + (level - 1) / 4
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbilityScores(pub [u8; 6]);

impl AbilityScores {
    pub const fn uniform(v: u8) -> Self {
        Self([v; 6])
    }
    pub fn new(str: u8, dex: u8, con: u8, int: u8, wis: u8, cha: u8) -> Self {
        Self([str, dex, con, int, wis, cha])
    }
    pub fn get(&self, a: Ability) -> u8 {
        self.0[a.index()]
    }
    pub fn set(&mut self, a: Ability, v: u8) {
        self.0[a.index()] = v;
    }
    pub fn modifier(&self, a: Ability) -> i32 {
        ability_modifier(self.get(a))
    }
}

impl Default for AbilityScores {
    fn default() -> Self {
        Self::uniform(10)
    }
}

/// The 18 skills of the 2024 rules and their default abilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Skill {
    Acrobatics,
    AnimalHandling,
    Arcana,
    Athletics,
    Deception,
    History,
    Insight,
    Intimidation,
    Investigation,
    Medicine,
    Nature,
    Perception,
    Performance,
    Persuasion,
    Religion,
    SleightOfHand,
    Stealth,
    Survival,
}

impl Skill {
    pub const ALL: [Skill; 18] = [
        Skill::Acrobatics,
        Skill::AnimalHandling,
        Skill::Arcana,
        Skill::Athletics,
        Skill::Deception,
        Skill::History,
        Skill::Insight,
        Skill::Intimidation,
        Skill::Investigation,
        Skill::Medicine,
        Skill::Nature,
        Skill::Perception,
        Skill::Performance,
        Skill::Persuasion,
        Skill::Religion,
        Skill::SleightOfHand,
        Skill::Stealth,
        Skill::Survival,
    ];

    pub fn ability(self) -> Ability {
        use Ability::*;
        use Skill::*;
        match self {
            Athletics => Strength,
            Acrobatics | SleightOfHand | Stealth => Dexterity,
            Arcana | History | Investigation | Nature | Religion => Intelligence,
            AnimalHandling | Insight | Medicine | Perception | Survival => Wisdom,
            Deception | Intimidation | Performance | Persuasion => Charisma,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifiers_match_srd_table() {
        let table = [
            (1, -5),
            (2, -4),
            (3, -4),
            (8, -1),
            (9, -1),
            (10, 0),
            (11, 0),
            (12, 1),
            (15, 2),
            (20, 5),
            (30, 10),
        ];
        for (score, m) in table {
            assert_eq!(ability_modifier(score), m, "score {score}");
        }
    }

    #[test]
    fn proficiency_bonus_by_level() {
        let expect = [2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 6];
        for (i, pb) in expect.iter().enumerate() {
            assert_eq!(proficiency_bonus(i as u8 + 1), *pb);
        }
    }

    #[test]
    fn skills_map_to_expected_abilities() {
        assert_eq!(Skill::Athletics.ability(), Ability::Strength);
        assert_eq!(Skill::Stealth.ability(), Ability::Dexterity);
        assert_eq!(Skill::Perception.ability(), Ability::Wisdom);
        assert_eq!(Skill::Persuasion.ability(), Ability::Charisma);
        assert_eq!(Skill::ALL.len(), 18);
    }
}
