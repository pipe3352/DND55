//! Attack resolution: attack roll vs AC, natural 1/20, Critical Hits, damage, defenses and the
//! weapon mastery properties that are fully automatable (Graze, Sap, Vex).
//! (SRD 5.2.1 "Making an Attack", "Rolling 20 or 1", "Critical Hits", "Mastery Properties")

use crate::d20::{D20Roll, ModeSources, RollMode};
use crate::damage::{DamageOutcome, DamageType, Defenses};
use crate::dice::{DiceExpr, DiceRoll};
use crate::equipment::{Mastery, WeaponKind};
use crate::rng::DiceRng;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamagePart {
    /// Dice plus flat modifier, e.g. `1d8+3`.
    pub dice: DiceExpr,
    pub damage_type: DamageType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackProfile {
    pub name: String,
    pub kind: WeaponKind,
    pub to_hit: i32,
    pub damage: Vec<DamagePart>,
    /// Extra damage dealt only "if the attack roll had Advantage" (e.g. Goblin Warrior's Scimitar).
    pub extra_damage_with_advantage: Vec<DamagePart>,
    pub reach_ft: u32,
    /// Normal/long range for ranged or thrown attacks.
    pub range_ft: Option<(u32, u32)>,
    /// Mastery property usable by this attacker with this weapon (None if not mastered).
    pub mastery: Option<Mastery>,
    /// Ability modifier used for the attack roll (needed by Graze and Topple).
    pub ability_mod: i32,
}

impl AttackProfile {
    /// Text like `Longsword +5, 1d8+3 Slashing`.
    pub fn summary(&self) -> String {
        let dmg: Vec<String> = self
            .damage
            .iter()
            .map(|d| format!("{} {:?}", d.dice, d.damage_type))
            .collect();
        format!("{} {:+}, {}", self.name, self.to_hit, dmg.join(" + "))
    }
}

#[derive(Debug, Clone, Default)]
pub struct AttackModifiers {
    pub sources: ModeSources,
    pub exhaustion_level: u8,
    /// Target is Paralyzed/Unconscious and the attacker is within 5 ft (see `conditions::hit_becomes_critical`).
    pub hit_is_auto_critical: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageDealt {
    pub roll: Option<DiceRoll>,
    pub outcome: DamageOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackResult {
    pub attack_name: String,
    pub d20: D20Roll,
    pub target_ac: i32,
    pub hit: bool,
    pub critical: bool,
    pub damage: Vec<DamageDealt>,
    pub total_damage: i32,
    pub advantage_reasons: Vec<String>,
    pub disadvantage_reasons: Vec<String>,
    /// Mastery effect that triggered, for the session layer to apply (Sap/Vex need tracking over time).
    pub mastery_triggered: Option<Mastery>,
}

impl AttackResult {
    pub fn log_line(&self, attacker: &str, target: &str) -> String {
        let outcome = if self.critical {
            "CRITICAL HIT".to_string()
        } else if self.hit {
            "hit".to_string()
        } else {
            "miss".to_string()
        };
        let mut s = format!(
            "{attacker} attacks {target} with {}: {} vs AC {} → {outcome}",
            self.attack_name,
            self.d20.breakdown(),
            self.target_ac
        );
        if !self.advantage_reasons.is_empty() || !self.disadvantage_reasons.is_empty() {
            s.push_str(&format!(
                " [adv: {:?}; dis: {:?}]",
                self.advantage_reasons, self.disadvantage_reasons
            ));
        }
        for d in &self.damage {
            let o = &d.outcome;
            let flags = [
                (o.immune, "immune"),
                (o.resisted, "resisted"),
                (o.vulnerable, "vulnerable"),
            ]
            .iter()
            .filter(|f| f.0)
            .map(|f| f.1)
            .collect::<Vec<_>>()
            .join(",");
            match &d.roll {
                Some(r) => s.push_str(&format!(
                    "; {} → {} {:?}",
                    r.breakdown(),
                    o.final_amount,
                    o.damage_type
                )),
                None => s.push_str(&format!("; {} {:?}", o.final_amount, o.damage_type)),
            }
            if !flags.is_empty() {
                s.push_str(&format!(" ({flags})"));
            }
        }
        if let Some(m) = self.mastery_triggered {
            s.push_str(&format!("; mastery: {m:?}"));
        }
        s
    }
}

/// Resolves one attack roll and its damage. Does not mutate any creature; the caller applies
/// `total_damage` with `HitPoints::take_damage(total, critical)`.
pub fn resolve_attack(
    rng: &mut dyn DiceRng,
    profile: &AttackProfile,
    target_ac: i32,
    target_defenses: &Defenses,
    mods: &AttackModifiers,
) -> AttackResult {
    let mode = mods.sources.mode();
    let d20 = D20Roll::roll(rng, mode, profile.to_hit, mods.exhaustion_level);
    // Rolling 20 or 1: natural 20 always hits (Critical Hit); natural 1 always misses.
    let hit = if d20.is_natural_20() {
        true
    } else if d20.is_natural_1() {
        false
    } else {
        d20.total >= target_ac
    };
    let critical = hit && (d20.is_natural_20() || mods.hit_is_auto_critical);

    let mut damage = Vec::new();
    let mut mastery_triggered = None;
    if hit {
        let mut parts: Vec<&DamagePart> = profile.damage.iter().collect();
        if mode == RollMode::Advantage {
            parts.extend(profile.extra_damage_with_advantage.iter());
        }
        for part in parts {
            let roll = part.dice.roll(rng, critical);
            let outcome = target_defenses.apply(part.damage_type, roll.total.max(0), 0);
            damage.push(DamageDealt {
                roll: Some(roll),
                outcome,
            });
        }
        let dealt: i32 = damage.iter().map(|d| d.outcome.final_amount).sum();
        mastery_triggered = match profile.mastery {
            // Sap: "If you hit a creature with this weapon".
            Some(Mastery::Sap) => Some(Mastery::Sap),
            // Vex: "If you hit a creature with this weapon and deal damage to the creature".
            Some(Mastery::Vex) if dealt > 0 => Some(Mastery::Vex),
            _ => None,
        };
    } else if profile.mastery == Some(Mastery::Graze) && profile.ability_mod > 0 {
        // Graze: on a miss, damage equal to the ability modifier used for the attack roll, same type.
        let dtype = profile
            .damage
            .first()
            .map(|d| d.damage_type)
            .unwrap_or(DamageType::Slashing);
        let outcome = target_defenses.apply(dtype, profile.ability_mod, 0);
        damage.push(DamageDealt {
            roll: None,
            outcome,
        });
        mastery_triggered = Some(Mastery::Graze);
    }
    let total_damage = damage.iter().map(|d| d.outcome.final_amount).sum();
    AttackResult {
        attack_name: profile.name.clone(),
        d20,
        target_ac,
        hit,
        critical,
        damage,
        total_damage,
        advantage_reasons: mods.sources.advantage.clone(),
        disadvantage_reasons: mods.sources.disadvantage.clone(),
        mastery_triggered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::ScriptedRng;

    fn longsword(mastery: Option<Mastery>) -> AttackProfile {
        AttackProfile {
            name: "Longsword".into(),
            kind: WeaponKind::Melee,
            to_hit: 5,
            damage: vec![DamagePart {
                dice: DiceExpr::dice(1, 8).with_modifier(3),
                damage_type: DamageType::Slashing,
            }],
            extra_damage_with_advantage: vec![],
            reach_ft: 5,
            range_ft: None,
            mastery,
            ability_mod: 3,
        }
    }

    #[test]
    fn hit_when_total_meets_ac() {
        let mut rng = ScriptedRng::new([10, 6]);
        let r = resolve_attack(
            &mut rng,
            &longsword(None),
            15,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert!(r.hit && !r.critical);
        assert_eq!(r.total_damage, 9);
    }

    #[test]
    fn miss_when_below_ac() {
        let mut rng = ScriptedRng::new([9]);
        let r = resolve_attack(
            &mut rng,
            &longsword(None),
            15,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert!(!r.hit);
        assert_eq!(r.total_damage, 0);
        assert_eq!(rng.remaining(), 0, "no damage dice rolled on a miss");
    }

    #[test]
    fn natural_20_always_hits_and_crits() {
        let mut rng = ScriptedRng::new([20, 8, 8]);
        let r = resolve_attack(
            &mut rng,
            &longsword(None),
            40,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert!(r.hit && r.critical);
        assert_eq!(r.total_damage, 8 + 8 + 3, "dice doubled, modifier once");
    }

    #[test]
    fn natural_1_always_misses() {
        let mut p = longsword(None);
        p.to_hit = 30;
        let r = resolve_attack(
            &mut ScriptedRng::new([1]),
            &p,
            5,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert!(!r.hit);
    }

    #[test]
    fn auto_critical_against_paralyzed_within_5ft() {
        let mods = AttackModifiers {
            hit_is_auto_critical: true,
            ..Default::default()
        };
        let r = resolve_attack(
            &mut ScriptedRng::new([12, 1, 1]),
            &longsword(None),
            10,
            &Defenses::default(),
            &mods,
        );
        assert!(r.critical);
        assert_eq!(r.total_damage, 5);
    }

    #[test]
    fn graze_on_miss_and_sap_vex_on_hit() {
        let mut gs = longsword(Some(Mastery::Graze));
        gs.name = "Greatsword".into();
        let r = resolve_attack(
            &mut ScriptedRng::new([2]),
            &gs,
            15,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert!(!r.hit);
        assert_eq!(
            (r.total_damage, r.mastery_triggered),
            (3, Some(Mastery::Graze))
        );

        let r = resolve_attack(
            &mut ScriptedRng::new([15, 4]),
            &longsword(Some(Mastery::Sap)),
            12,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert_eq!(r.mastery_triggered, Some(Mastery::Sap));

        // Vex requires dealing damage: an immune target takes 0, so no Vex.
        let immune = Defenses {
            immunities: [DamageType::Slashing].into(),
            ..Default::default()
        };
        let r = resolve_attack(
            &mut ScriptedRng::new([15, 4]),
            &longsword(Some(Mastery::Vex)),
            12,
            &immune,
            &AttackModifiers::default(),
        );
        assert!(r.hit);
        assert_eq!(r.mastery_triggered, None);
    }

    #[test]
    fn extra_damage_only_with_advantage() {
        let mut p = longsword(None);
        p.extra_damage_with_advantage = vec![DamagePart {
            dice: DiceExpr::dice(1, 4),
            damage_type: DamageType::Slashing,
        }];
        let mut mods = AttackModifiers::default();
        mods.sources.adv("test");
        let r = resolve_attack(
            &mut ScriptedRng::new([3, 15, 5, 2]),
            &p,
            12,
            &Defenses::default(),
            &mods,
        );
        assert_eq!(r.total_damage, 5 + 3 + 2);
        let r = resolve_attack(
            &mut ScriptedRng::new([15, 5]),
            &p,
            12,
            &Defenses::default(),
            &AttackModifiers::default(),
        );
        assert_eq!(r.total_damage, 8);
    }
}
