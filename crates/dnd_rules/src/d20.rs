//! D20 Tests: ability checks, saving throws and attack rolls. (SRD 5.2.1 "D20 Tests", "Advantage/Disadvantage", "Exhaustion")

use crate::rng::DiceRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum RollMode {
    #[default]
    Normal,
    Advantage,
    Disadvantage,
}

impl RollMode {
    /// "A roll can't be affected by more than one Advantage, and Advantage and Disadvantage
    /// on the same roll cancel each other." — so any number of sources collapses to a boolean each.
    pub fn from_sources(advantage_sources: usize, disadvantage_sources: usize) -> Self {
        match (advantage_sources > 0, disadvantage_sources > 0) {
            (true, false) => RollMode::Advantage,
            (false, true) => RollMode::Disadvantage,
            _ => RollMode::Normal,
        }
    }
}

/// Collected reasons for advantage/disadvantage, kept so the combat log can explain the roll.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModeSources {
    pub advantage: Vec<String>,
    pub disadvantage: Vec<String>,
}

impl ModeSources {
    pub fn adv(&mut self, why: impl Into<String>) {
        self.advantage.push(why.into());
    }
    pub fn dis(&mut self, why: impl Into<String>) {
        self.disadvantage.push(why.into());
    }
    pub fn mode(&self) -> RollMode {
        RollMode::from_sources(self.advantage.len(), self.disadvantage.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct D20Roll {
    pub mode: RollMode,
    /// One die normally, two with advantage/disadvantage.
    pub dice: Vec<u32>,
    /// The die that counts (natural value).
    pub natural: u32,
    pub modifier: i32,
    /// "the roll is reduced by 2 times your Exhaustion level" — stored separately for the log.
    pub exhaustion_penalty: i32,
    pub total: i32,
}

impl D20Roll {
    pub fn roll(
        rng: &mut dyn DiceRng,
        mode: RollMode,
        modifier: i32,
        exhaustion_level: u8,
    ) -> Self {
        let first = rng.roll_die(20);
        let (dice, natural) = match mode {
            RollMode::Normal => (vec![first], first),
            RollMode::Advantage => {
                let second = rng.roll_die(20);
                (vec![first, second], first.max(second))
            }
            RollMode::Disadvantage => {
                let second = rng.roll_die(20);
                (vec![first, second], first.min(second))
            }
        };
        let exhaustion_penalty = 2 * exhaustion_level as i32;
        D20Roll {
            mode,
            dice,
            natural,
            modifier,
            exhaustion_penalty,
            total: natural as i32 + modifier - exhaustion_penalty,
        }
    }

    pub fn is_natural_20(&self) -> bool {
        self.natural == 20
    }
    pub fn is_natural_1(&self) -> bool {
        self.natural == 1
    }

    pub fn breakdown(&self) -> String {
        let dice = match self.mode {
            RollMode::Normal => format!("d20[{}]", self.natural),
            RollMode::Advantage => format!("d20 adv{:?}→{}", self.dice, self.natural),
            RollMode::Disadvantage => format!("d20 dis{:?}→{}", self.dice, self.natural),
        };
        let exh = if self.exhaustion_penalty > 0 {
            format!(" -{} (exhaustion)", self.exhaustion_penalty)
        } else {
            String::new()
        };
        format!("{dice} {:+}{exh} = {}", self.modifier, self.total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::ScriptedRng;

    #[test]
    fn advantage_and_disadvantage_cancel_and_do_not_stack() {
        assert_eq!(RollMode::from_sources(0, 0), RollMode::Normal);
        assert_eq!(RollMode::from_sources(3, 0), RollMode::Advantage);
        assert_eq!(RollMode::from_sources(0, 2), RollMode::Disadvantage);
        // Three sources of advantage and one of disadvantage still cancel to a normal roll.
        assert_eq!(RollMode::from_sources(3, 1), RollMode::Normal);
    }

    #[test]
    fn advantage_takes_higher_disadvantage_lower() {
        let r = D20Roll::roll(&mut ScriptedRng::new([4, 17]), RollMode::Advantage, 3, 0);
        assert_eq!((r.natural, r.total), (17, 20));
        let r = D20Roll::roll(&mut ScriptedRng::new([4, 17]), RollMode::Disadvantage, 3, 0);
        assert_eq!((r.natural, r.total), (4, 7));
    }

    #[test]
    fn exhaustion_reduces_d20_tests_by_two_per_level() {
        let r = D20Roll::roll(&mut ScriptedRng::new([15]), RollMode::Normal, 2, 3);
        assert_eq!(r.total, 15 + 2 - 6);
    }
}
