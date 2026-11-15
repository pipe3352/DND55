//! Damage types, Resistance, Vulnerability and Immunity. (SRD 5.2.1 "Damage and Healing")

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DamageType {
    Acid,
    Bludgeoning,
    Cold,
    Fire,
    Force,
    Lightning,
    Necrotic,
    Piercing,
    Poison,
    Psychic,
    Radiant,
    Slashing,
    Thunder,
}

impl DamageType {
    pub const ALL: [DamageType; 13] = [
        DamageType::Acid,
        DamageType::Bludgeoning,
        DamageType::Cold,
        DamageType::Fire,
        DamageType::Force,
        DamageType::Lightning,
        DamageType::Necrotic,
        DamageType::Piercing,
        DamageType::Poison,
        DamageType::Psychic,
        DamageType::Radiant,
        DamageType::Slashing,
        DamageType::Thunder,
    ];
}

/// A creature's damage defenses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defenses {
    pub resistances: BTreeSet<DamageType>,
    pub vulnerabilities: BTreeSet<DamageType>,
    pub immunities: BTreeSet<DamageType>,
    /// Resistance to all damage (e.g. the Petrified condition).
    pub resist_all: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageOutcome {
    pub damage_type: DamageType,
    pub rolled: i32,
    pub final_amount: i32,
    pub resisted: bool,
    pub vulnerable: bool,
    pub immune: bool,
}

impl Defenses {
    pub fn resists(&self, t: DamageType) -> bool {
        self.resist_all || self.resistances.contains(&t)
    }

    /// Applies, in SRD order: (1) adjustments (bonuses, penalties), (2) Resistance (halve, round down),
    /// (3) Vulnerability (double). Immunity means no damage. Multiple instances of Resistance or
    /// Vulnerability to the same type count only once ("No Stacking"). Damage is never negative.
    pub fn apply(&self, damage_type: DamageType, rolled: i32, adjustment: i32) -> DamageOutcome {
        let immune = self.immunities.contains(&damage_type);
        let resisted = !immune && self.resists(damage_type);
        let vulnerable = !immune && self.vulnerabilities.contains(&damage_type);
        let mut amount = (rolled + adjustment).max(0);
        if immune {
            amount = 0;
        }
        if resisted {
            amount /= 2; // non-negative, so integer division rounds down
        }
        if vulnerable {
            amount *= 2;
        }
        DamageOutcome {
            damage_type,
            rolled,
            final_amount: amount,
            resisted,
            vulnerable,
            immune,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srd_order_of_application_example() {
        // SRD example: Resistance to all damage, Vulnerability to Fire, an aura reducing all damage by 5.
        // 28 Fire damage -> 23 -> 11 (round down) -> 22.
        let d = Defenses {
            resist_all: true,
            vulnerabilities: [DamageType::Fire].into(),
            ..Default::default()
        };
        let o = d.apply(DamageType::Fire, 28, -5);
        assert_eq!(o.final_amount, 22);
        assert!(o.resisted && o.vulnerable);
    }

    #[test]
    fn resistance_does_not_stack_and_rounds_down() {
        let d = Defenses {
            resist_all: true,
            resistances: [DamageType::Necrotic].into(),
            ..Default::default()
        };
        assert_eq!(d.apply(DamageType::Necrotic, 9, 0).final_amount, 4);
    }

    #[test]
    fn immunity_and_vulnerability() {
        let d = Defenses {
            immunities: [DamageType::Poison].into(),
            vulnerabilities: [DamageType::Bludgeoning].into(),
            ..Default::default()
        };
        assert_eq!(d.apply(DamageType::Poison, 30, 0).final_amount, 0);
        assert_eq!(d.apply(DamageType::Bludgeoning, 7, 0).final_amount, 14);
        assert_eq!(d.apply(DamageType::Slashing, 2, -5).final_amount, 0);
    }
}
