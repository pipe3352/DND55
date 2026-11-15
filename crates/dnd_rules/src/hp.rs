//! Hit Points, Temporary Hit Points, healing, dropping to 0 HP and Death Saving Throws.
//! (SRD 5.2.1 "Damage and Healing", "Dropping to 0 Hit Points", "Death Saving Throws", "Temporary Hit Points")

use serde::{Deserialize, Serialize};

/// How a creature is treated when it drops to 0 Hit Points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ZeroHpRule {
    /// Player characters (and monsters the GM chooses to treat like characters): fall Unconscious and make Death Saving Throws.
    Character,
    /// "A monster dies the instant it drops to 0 Hit Points".
    MonsterDies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DeathSaves {
    pub successes: u8,
    pub failures: u8,
    pub stable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LifeState {
    Conscious,
    /// At 0 HP, Unconscious, making Death Saving Throws.
    Dying,
    /// At 0 HP, Unconscious, no longer making Death Saving Throws.
    Stable,
    Dead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HitPoints {
    pub current: i32,
    pub max: i32,
    pub temp: i32,
    pub death_saves: DeathSaves,
    pub dead: bool,
    pub zero_hp_rule: ZeroHpRule,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DamageApplied {
    pub absorbed_by_temp: i32,
    pub hp_lost: i32,
    pub dropped_to_zero: bool,
    pub died: bool,
    pub death_save_failures_added: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathSaveResult {
    Success,
    Failure,
    /// Natural 1: two failures.
    DoubleFailure,
    /// Natural 20: regain 1 Hit Point.
    RegainedHp,
    BecameStable,
    Died,
}

impl HitPoints {
    pub fn new(max: i32, zero_hp_rule: ZeroHpRule) -> Self {
        Self { current: max, max, temp: 0, death_saves: DeathSaves::default(), dead: false, zero_hp_rule }
    }

    pub fn life_state(&self) -> LifeState {
        if self.dead {
            LifeState::Dead
        } else if self.current > 0 {
            LifeState::Conscious
        } else if self.death_saves.stable {
            LifeState::Stable
        } else {
            LifeState::Dying
        }
    }

    /// "A creature is Bloodied while it has half its Hit Points or fewer remaining."
    pub fn is_bloodied(&self) -> bool {
        self.current * 2 <= self.max
    }

    /// Applies final (post-defense) damage.
    pub fn take_damage(&mut self, amount: i32, from_critical_hit: bool) -> DamageApplied {
        let mut out = DamageApplied::default();
        if self.dead || amount <= 0 {
            return out;
        }
        // Lose Temporary Hit Points First.
        let absorbed = amount.min(self.temp);
        self.temp -= absorbed;
        out.absorbed_by_temp = absorbed;
        let remaining = amount - absorbed;
        if remaining == 0 {
            return out;
        }

        if self.current == 0 {
            // Damage at 0 Hit Points.
            if remaining >= self.max {
                self.dead = true;
                out.died = true;
                return out;
            }
            let fails = if from_critical_hit { 2 } else { 1 };
            self.death_saves.stable = false;
            self.death_saves.failures = (self.death_saves.failures + fails).min(3);
            out.death_save_failures_added = fails;
            if self.death_saves.failures >= 3 {
                self.dead = true;
                out.died = true;
            }
            return out;
        }

        let lost = remaining.min(self.current);
        self.current -= lost;
        out.hp_lost = lost;
        if self.current == 0 {
            out.dropped_to_zero = true;
            let overflow = remaining - lost;
            match self.zero_hp_rule {
                ZeroHpRule::MonsterDies => {
                    self.dead = true;
                    out.died = true;
                }
                ZeroHpRule::Character => {
                    // Massive Damage.
                    if overflow >= self.max {
                        self.dead = true;
                        out.died = true;
                    } else {
                        self.death_saves = DeathSaves::default();
                    }
                }
            }
        }
        out
    }

    /// Healing; excess over the maximum is lost. Regaining any HP resets Death Saving Throws.
    /// Returns HP actually regained. The dead cannot be healed by ordinary healing.
    pub fn heal(&mut self, amount: i32) -> i32 {
        if self.dead || amount <= 0 {
            return 0;
        }
        let before = self.current;
        self.current = (self.current + amount).min(self.max);
        if self.current > 0 {
            self.death_saves = DeathSaves::default();
        }
        self.current - before
    }

    /// "Temporary Hit Points can't be added together ... you decide whether to keep the ones you have
    /// or to gain the new ones." The caller passes the creature's decision.
    pub fn offer_temp_hp(&mut self, amount: i32, accept_new: bool) {
        if accept_new {
            self.temp = amount.max(0);
        }
    }

    /// Resolves a Death Saving Throw (DC 10). `natural` decides the 1/20 special cases; `total` (the d20
    /// after modifiers such as Exhaustion, which applies to every D20 Test) is compared with the DC.
    pub fn death_saving_throw_total(&mut self, natural: u32, total: i32) -> DeathSaveResult {
        debug_assert_eq!(self.life_state(), LifeState::Dying);
        if natural == 20 {
            self.heal(1);
            return DeathSaveResult::RegainedHp;
        }
        let result = if natural == 1 {
            self.death_saves.failures += 2;
            DeathSaveResult::DoubleFailure
        } else if total >= 10 {
            self.death_saves.successes += 1;
            DeathSaveResult::Success
        } else {
            self.death_saves.failures += 1;
            DeathSaveResult::Failure
        };
        if self.death_saves.failures >= 3 {
            self.death_saves.failures = 3;
            self.dead = true;
            return DeathSaveResult::Died;
        }
        if self.death_saves.successes >= 3 {
            // On your third success you become Stable; both counts reset when you become Stable.
            self.death_saves = DeathSaves { successes: 0, failures: 0, stable: true };
            return DeathSaveResult::BecameStable;
        }
        result
    }

    /// Unmodified Death Saving Throw from the natural d20 value.
    pub fn death_saving_throw(&mut self, natural: u32) -> DeathSaveResult {
        self.death_saving_throw_total(natural, natural as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temp_hp_absorbs_first() {
        // SRD: 5 Temporary Hit Points and 7 damage -> lose the temp points, then 2 Hit Points.
        let mut hp = HitPoints::new(20, ZeroHpRule::Character);
        hp.offer_temp_hp(5, true);
        let r = hp.take_damage(7, false);
        assert_eq!((r.absorbed_by_temp, r.hp_lost, hp.current, hp.temp), (5, 2, 18, 0));
    }

    #[test]
    fn temp_hp_do_not_stack() {
        let mut hp = HitPoints::new(20, ZeroHpRule::Character);
        hp.offer_temp_hp(10, true);
        hp.offer_temp_hp(12, true);
        assert_eq!(hp.temp, 12);
        hp.offer_temp_hp(3, false);
        assert_eq!(hp.temp, 12);
    }

    #[test]
    fn massive_damage_srd_example() {
        // HP max 12, current 6, takes 18: drops to 0 with 12 remaining -> dies.
        let mut hp = HitPoints::new(12, ZeroHpRule::Character);
        hp.current = 6;
        let r = hp.take_damage(18, false);
        assert!(r.died && hp.dead);
        // 17 damage leaves 11 remaining -> falls unconscious instead.
        let mut hp = HitPoints::new(12, ZeroHpRule::Character);
        hp.current = 6;
        let r = hp.take_damage(17, false);
        assert!(r.dropped_to_zero && !r.died);
        assert_eq!(hp.life_state(), LifeState::Dying);
    }

    #[test]
    fn monsters_die_at_zero() {
        let mut hp = HitPoints::new(7, ZeroHpRule::MonsterDies);
        let r = hp.take_damage(7, false);
        assert!(r.died);
        assert_eq!(hp.life_state(), LifeState::Dead);
    }

    #[test]
    fn healing_caps_at_max_srd_example() {
        let mut hp = HitPoints::new(20, ZeroHpRule::Character);
        hp.current = 14;
        assert_eq!(hp.heal(8), 6);
        assert_eq!(hp.current, 20);
    }

    #[test]
    fn damage_at_zero_hp_adds_failures() {
        let mut hp = HitPoints::new(10, ZeroHpRule::Character);
        hp.take_damage(10, false);
        assert_eq!(hp.take_damage(3, false).death_save_failures_added, 1);
        assert_eq!(hp.take_damage(3, true).death_save_failures_added, 2);
        assert!(hp.dead, "three failures kill");
        // Damage at 0 HP that equals the HP maximum kills outright.
        let mut hp = HitPoints::new(10, ZeroHpRule::Character);
        hp.take_damage(10, false);
        assert!(hp.take_damage(10, false).died);
    }

    #[test]
    fn death_saves_rules() {
        let mut hp = HitPoints::new(10, ZeroHpRule::Character);
        hp.take_damage(10, false);
        assert_eq!(hp.death_saving_throw(10), DeathSaveResult::Success);
        assert_eq!(hp.death_saving_throw(9), DeathSaveResult::Failure);
        assert_eq!(hp.death_saving_throw(15), DeathSaveResult::Success);
        assert_eq!(hp.death_saving_throw(12), DeathSaveResult::BecameStable);
        assert_eq!(hp.life_state(), LifeState::Stable);

        let mut hp = HitPoints::new(10, ZeroHpRule::Character);
        hp.take_damage(10, false);
        assert_eq!(hp.death_saving_throw(1), DeathSaveResult::DoubleFailure);
        assert_eq!(hp.death_saving_throw(2), DeathSaveResult::Died);

        let mut hp = HitPoints::new(10, ZeroHpRule::Character);
        hp.take_damage(10, false);
        hp.death_saving_throw(3);
        assert_eq!(hp.death_saving_throw(20), DeathSaveResult::RegainedHp);
        assert_eq!((hp.current, hp.death_saves.failures), (1, 0));
    }

    #[test]
    fn bloodied_at_half_or_less() {
        let mut hp = HitPoints::new(11, ZeroHpRule::Character);
        hp.current = 6;
        assert!(!hp.is_bloodied());
        hp.current = 5;
        assert!(hp.is_bloodied());
    }
}
